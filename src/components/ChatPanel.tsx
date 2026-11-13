// The AI chat with Lumen (F8): a panel next to the companion. Answers stream in from a local
// model through Ollama; links in an answer select items or open suggestion groups, never
// more. When the chat is off, the panel says how to switch it on.
import { memo, useEffect, useRef, useState, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, onChatEvent } from "../api/ipc";
import type { ChatError, ChatMessage, ModelInfo } from "../api/types";
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

/** The chat panel. */
// Memoised: it reads its state from the store, so a re-render of the app passes it by.
export const ChatPanel = memo(function ChatPanel(): JSX.Element | null {
  const { t } = useTranslation();
  const { open, setOpen, lang, settings, setScreen, select, setFocus } = useApp(
    useShallow((s) => ({ open: s.chatOpen, setOpen: s.setChatOpen, lang: s.lang, settings: s.settings, setScreen: s.setScreen, select: s.select, setFocus: s.setFocus })),
  );
  const [messages, setMessages] = useState<ChatMessage[]>([]);
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
          if (last?.role === "assistant") return [...m.slice(0, -1), { role: "assistant", content: last.content + e.text }];
          return [...m, { role: "assistant", content: e.text }];
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
  }, [messages, tool]);

  if (!open) return null;
  const model = settings.chatModel ?? models?.find((m) => m.recommended)?.name ?? "qwen3:8b";
  const missing = models !== null && !models.some((m) => m.name === model);

  const send = (): void => {
    const text = draft.trim();
    if (!text || busy) return;
    const next: ChatMessage[] = [...messages, { role: "user", content: text }];
    setMessages(next);
    setDraft("");
    setBusy(true);
    setProblem(null);
    api
      .chatSend(next, lang)
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
      {!settings.chat ? (
        <div className="chat-off">
          <p className="info">{t("chat.off")}</p>
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
          {problem ? <p className="note warn">{t(`chat.errors.${problem}`, { model })}</p> : null}
          {missing && !problem ? (
            <p className="note">
              {t("chat.missingModel", { model })} <code>{t("chat.pullCommand", { model })}</code>
            </p>
          ) : null}
          <div className="chat-log" aria-live="polite">
            {messages.length === 0 ? <p className="note">{t("chat.hello")}</p> : null}
            {messages.map((m, i) => (
              <div key={i} className={`msg ${m.role}`}>
                {m.role === "assistant"
                  ? parseAnswer(m.content).map((p, j) =>
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
                  : m.content}
              </div>
            ))}
            {tool ? <p className="note">{t("chat.looking", { tool: t(`chat.tools.${tool}`, { defaultValue: tool }) })}</p> : null}
            <div ref={endRef} />
          </div>
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
