// Lumen's panel (F8): everything Lumen said and the chat, in one timeline. Each tip shows
// briefly as a bubble at the orb and stays here, with its buttons. Answers stream in from a
// local model through Ollama; links in an answer select items or open suggestion groups,
// never more. When the chat is off, the tips still show and the panel says how to switch
// the chat on.
import { memo, useEffect, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, onChatEvent } from "../api/ipc";
import type { ChatError, ChatMessage, ModelInfo } from "../api/types";
import type { Bubble } from "../companion/queue";
import { useApp } from "../store";

/** One piece of an answer: text, or a link to an item or a suggestion group. */
type Part = { text: string } | { text: string; item: number } | { text: string; group: string };

/** Splits an answer into text and `unhoard:` links; other links stay text. */
export function parseAnswer(answer: string): Part[] {
  const out: Part[] = [];
  const re = /\[([^\]]{1,200})\]\(unhoard:(item|group)\/([^)\s]{1,300})\)/g;
  let last = 0;
  for (const m of answer.matchAll(re)) {
    const at = m.index;
    if (at > last) out.push({ text: answer.slice(last, at) });
    const [, label = "", kind, target = ""] = m;
    if (kind === "item" && /^\d+$/.test(target)) out.push({ text: label, item: Number(target) });
    else if (kind === "group") out.push({ text: label, group: target });
    else out.push({ text: label });
    last = at + m[0].length;
  }
  if (last < answer.length) out.push({ text: answer.slice(last) });
  return out;
}

/** A chat message with the moment it was written, so it sorts between Lumen's tips. */
type Line = ChatMessage & { at: number };

/** One entry of the timeline: a tip of Lumen's, or a chat message. */
type Entry = { kind: "tip"; at: number; bubble: Bubble } | { kind: "msg"; at: number; line: Line; index: number };

/** Lumen's panel. */
// Memoised: it reads its state from the store, so a re-render of the app passes it by.
export const ChatPanel = memo(function ChatPanel(): JSX.Element | null {
  const { t } = useTranslation();
  const { open, setOpen, lang, settings, setScreen, select, setFocus, queue, bubbleVersion } = useApp(
    useShallow((s) => ({
      open: s.chatOpen,
      setOpen: s.setChatOpen,
      lang: s.lang,
      settings: s.settings,
      setScreen: s.setScreen,
      select: s.select,
      setFocus: s.setFocus,
      queue: s.queue,
      bubbleVersion: s.bubbleVersion,
    })),
  );
  const [messages, setMessages] = useState<Line[]>([]);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [tool, setTool] = useState<string | null>(null);
  const [problem, setProblem] = useState<ChatError | null>(null);
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open || !settings.chat) return;
    api
      .chatModels()
      .then((r) => {
        if ("Ok" in r) {
          setModels(r.Ok);
          setProblem(null);
        } else setProblem(r.Err);
      })
      .catch(() => {
        setProblem("badReply");
      });
  }, [open, settings.chat]);

  useEffect(() => {
    const off = onChatEvent((e) => {
      if (e.kind === "delta") {
        setTool(null);
        setMessages((m) => {
          const last = m[m.length - 1];
          if (last?.role === "assistant") return [...m.slice(0, -1), { ...last, content: last.content + e.text }];
          return [...m, { role: "assistant", content: e.text, at: Date.now() }];
        });
      } else if (e.kind === "tool") setTool(e.name);
      else setTool(null);
    });
    return () => {
      off.then((f) => {
        f();
      }).catch(() => undefined);
    };
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [messages, tool, bubbleVersion, open]);

  if (!open) return null;
  const model = settings.chatModel ?? models?.find((m) => m.recommended)?.name ?? "qwen3:8b";
  const missing = models !== null && !models.some((m) => m.name === model);

  const send = (): void => {
    const text = draft.trim();
    if (!text || busy) return;
    const next: Line[] = [...messages, { role: "user", content: text, at: Date.now() }];
    setMessages(next);
    setDraft("");
    setBusy(true);
    setProblem(null);
    api
      .chatSend(
        next.map((m) => ({ role: m.role, content: m.content })),
        lang,
      )
      .then((r) => {
        if ("Err" in r) setProblem(r.Err);
      })
      .catch(() => {
        setProblem("badReply");
      })
      .finally(() => {
        setBusy(false);
        setTool(null);
      });
  };
  // Every tip Lumen said, without progress updates, then the chat, in the order they came.
  const tips = [...queue.history, ...(queue.current && queue.current.type !== "progress" ? [queue.current] : [])].slice(-30);
  const timeline: Entry[] = [
    ...tips.map((b): Entry => ({ kind: "tip", at: b.at, bubble: b })),
    ...messages.map((line, index): Entry => ({ kind: "msg", at: line.at, line, index })),
  ].sort((a, b) => a.at - b.at);
  const follow = (p: Part): void => {
    if ("item" in p) {
      setFocus(0);
      select(p.item);
      setScreen("overview");
    } else if ("group" in p) setScreen("suggestions");
  };

  return (
    <aside className="chat panel" aria-label={t("chat.title")}>
      <div className="row-head">
        <b>{t("chat.title")}</b>
        <button
          className="btn small"
          aria-label={t("chat.close")}
          onClick={() => {
            setOpen(false);
          }}
        >
          ×
        </button>
      </div>
      {settings.chat && problem ? <p className="note warn">{t(`chat.errors.${problem}`, { model })}</p> : null}
      {settings.chat && missing && !problem ? <p className="note">{t("chat.missingModel", { model })}</p> : null}
      {settings.chat && (problem === "notRunning" || problem === "refused" || (missing && !problem)) ? (
        <button
          className="btn small"
          style={{ alignSelf: "flex-start" }}
          onClick={() => {
            setScreen("settings");
          }}
        >
          {t("chat.setup.toSettings")}
        </button>
      ) : null}
      <div className="chat-log" aria-live="polite">
        {timeline.length === 0 ? <p className="note">{t("chat.hello")}</p> : null}
        {timeline.map((e) =>
          e.kind === "tip" ? (
            <div key={`tip-${String(e.bubble.id)}`} className={`msg assistant tip ${e.bubble.type}`}>
              <div className="kind">{t(`companion.types.${e.bubble.type}`)}</div>
              <div>{e.bubble.text}</div>
              {e.bubble.actions ? (
                <div className="btns">
                  {e.bubble.actions.map((a) => (
                    <button key={a.label} className="btn small" onClick={a.run}>
                      {a.label}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : (
            <div key={`msg-${String(e.index)}`} className={`msg ${e.line.role}`}>
              {e.line.role === "assistant"
                ? parseAnswer(e.line.content).map((p, j) =>
                    "item" in p || "group" in p ? (
                      <button
                        key={j}
                        className="link"
                        onClick={() => {
                          follow(p);
                        }}
                      >
                        {p.text}
                      </button>
                    ) : (
                      <span key={j}>{p.text}</span>
                    ),
                  )
                : e.line.content}
            </div>
          ),
        )}
        {tool ? <p className="note">{t("chat.looking", { tool: t(`chat.tools.${tool}`, { defaultValue: tool }) })}</p> : null}
        <div ref={endRef} />
      </div>
      {!settings.chat ? (
        <div className="chat-off">
          <p className="note">{t("chat.off")}</p>
          <button
            className="btn small"
            onClick={() => {
              setScreen("settings");
            }}
          >
            {t("chat.toSettings")}
          </button>
        </div>
      ) : (
        <>
          <form
            className="chat-input"
            onSubmit={(e) => {
              e.preventDefault();
              send();
            }}
          >
            <textarea
              value={draft}
              rows={2}
              maxLength={4000}
              placeholder={t("chat.placeholder")}
              onChange={(e) => {
                setDraft(e.target.value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  send();
                }
              }}
            />
            {busy ? (
              <button
                type="button"
                className="btn small"
                onClick={() => {
                  api.chatStop().catch(() => undefined);
                }}
              >
                {t("chat.stop")}
              </button>
            ) : (
              <button type="submit" className="btn small primary" disabled={!draft.trim()}>
                {t("chat.send")}
              </button>
            )}
          </form>
          <p className="note small">{t("chat.privacy", { model })}</p>
        </>
      )}
    </aside>
  );
});
