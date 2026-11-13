// The app shell: navigation, top bar, the screens, the drive bar and the first-run
// welcome. The shell stays calm; the stage carries the motion.
import { memo, useEffect, type JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import { api, onScanProgress } from "./api/ipc";
import { Lumen } from "./components/Companion";
import { DetailsPanel, ItemList } from "./components/SidePanels";
import { Stage, useView } from "./components/Stage";
import { fmtBytes, fmtInt } from "./format";
import { Cloud } from "./screens/Cloud";
import { Photos } from "./screens/Photos";
import { Quarantine, useQuarantine } from "./screens/Quarantine";
import { Settings } from "./screens/Settings";
import { Suggestions } from "./screens/Suggestions";
import { measureSince } from "./perf/marks";
import { ChatPanel } from "./components/ChatPanel";
import { History } from "./screens/History";
import { Optimise } from "./screens/Optimise";
import { useApp, type Screen } from "./store";

const ICONS: Record<Screen, JSX.Element> = {
  overview: (
    <svg viewBox="0 0 24 24">
      <circle cx="9" cy="10" r="6" />
      <circle cx="17" cy="15" r="4" />
      <circle cx="8" cy="19" r="2" />
    </svg>
  ),
  suggestions: (
    <svg viewBox="0 0 24 24">
      <path d="M12 3a6 6 0 0 0-3.5 10.9V17h7v-3.1A6 6 0 0 0 12 3Z" />
      <path d="M9.5 21h5" />
    </svg>
  ),
  history: (
    <svg viewBox="0 0 24 24">
      <path d="M3 17l5-6 4 3 6-8 3 3" />
      <path d="M3 21h18" />
    </svg>
  ),
  optimise: (
    <svg viewBox="0 0 24 24">
      <path d="M4 14a8 8 0 0 1 16 0" />
      <path d="M12 14l4-5" />
      <circle cx="12" cy="14" r="1.5" />
    </svg>
  ),
  quarantine: (
    <svg viewBox="0 0 24 24">
      <rect x="3" y="7" width="18" height="13" rx="2" />
      <path d="M3 11h18M9 4h6l1 3H8z" />
    </svg>
  ),
  cloud: (
    <svg viewBox="0 0 24 24">
      <path d="M7 18h10a4 4 0 0 0 .5-8A6 6 0 0 0 6 9.5 4.3 4.3 0 0 0 7 18Z" />
    </svg>
  ),
  photos: (
    <svg viewBox="0 0 24 24">
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <circle cx="9" cy="10" r="2" />
      <path d="m21 16-5-5-8 8" />
    </svg>
  ),
  settings: (
    <svg viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" />
    </svg>
  ),
};
/** How often the scan's progress bubble updates its count, in place. */
const PROGRESS_BUBBLE_MS = 2000;
const SCREENS: Screen[] = ["overview", "suggestions", "history", "optimise", "quarantine", "cloud", "photos", "settings"];

type T = (k: string, o?: Record<string, unknown>) => string;

/** Reads every folder the scan could not read with the helper started for it (AC3.7). */
function readProtected(generation: number, t: T): void {
  const st = useApp.getState();
  st.setAdminArmed(false);
  api
    .adminRead(generation, null)
    .then((r) => {
      if ("Ok" in r && r.Ok.folders > 0) {
        st.say({ type: "explanation", text: t("companion.script.adminReadDone.0", { count: r.Ok.folders, size: fmtBytes(r.Ok.onDisk, st.lang) }) });
        st.treeChanged();
      } else if ("Err" in r && r.Err.kind !== "declined") {
        st.say({ type: "warning", text: t("errors.invalidArgument") });
      }
    })
    .catch((e: unknown) => {
      st.sayError(e, t);
    });
}

/** Starts the elevated helper (one UAC prompt per app session; later scans reuse it); it
 * reads when the scan ends. */
function armAdmin(t: T): void {
  const st = useApp.getState();
  api
    .adminStart()
    .then((r) => {
      if ("Ok" in r && r.Ok) {
        st.setAdminAnswer("yes");
        st.setAdminArmed(true);
        // The scan may already have ended while the question waited.
        const scan = useApp.getState().scan;
        if (scan?.done) readProtected(scan.generation, t);
      } else if ("Ok" in r) {
        st.setAdminAnswer("no");
        st.say({ type: "explanation", text: t("companion.script.revealDeclined.0") });
      }
    })
    .catch((e: unknown) => {
      st.sayError(e, t);
    });
}

/** After a scan, names the largest grower of the last week when it grew by 5 GB or more. */
function tellGrowth(drive: string, t: T): void {
  api
    .historyGrowth(drive, 7)
    .then((g) => {
      const top = g.items[0];
      const st = useApp.getState();
      if (!top || top.delta < 5 * 1024 ** 3) return;
      st.say({
        type: "tip",
        text: t("companion.script.growthFound.0", {
          name: top.ruleId ? t(`knownLocations.${top.ruleId}.title`) : top.name,
          size: fmtBytes(top.delta, st.lang),
        }),
        actions: [
          {
            label: t("companion.openHistory"),
            run: () => {
              useApp.getState().setScreen("history");
            },
          },
        ],
      });
    })
    .catch(() => undefined);
}

function startScan(drive: string, t: T): void {
  const st = useApp.getState();
  st.endTopic("scan");
  st.endTopic("admin");
  st.setAdminArmed(false);
  // Protected folders: ask once per app session, read always, or never (AC3.6, AC3.7). A
  // "yes" keeps the helper running, so later scans read without a new question or prompt.
  if (st.settings.adminRead === "always" || (st.settings.adminRead === "ask" && st.adminAnswer === "yes")) armAdmin(t);
  else if (st.settings.adminRead === "ask" && st.adminAnswer === null) {
    st.say({
      type: "explanation",
      key: "admin",
      text: t("companion.script.adminAsk.0"),
      actions: [
        {
          label: t("companion.adminYes"),
          run: () => {
            armAdmin(t);
          },
        },
        {
          label: t("companion.adminNo"),
          run: () => {
            useApp.getState().setAdminAnswer("no");
          },
        },
      ],
    });
  }
  st.say({ type: "progress", text: t("companion.script.scanStart.0", { drive }), key: "scan" });
  api
    .scanStart(drive)
    .then(() => {
      st.setFocus(0);
    })
    .catch((e: unknown) => {
      st.sayError(e, t);
    });
}

/** The overview; memoised so app-level changes (settings, search) do not re-render it. */
const Overview = memo(function Overview({ active }: { active: boolean }): JSX.Element {
  const resp = useView();
  return (
    <div className="overview" hidden={!active}>
      <Stage resp={resp} active={active} />
      <div className="side">
        <ItemList resp={resp} />
        <DetailsPanel resp={resp} />
      </div>
    </div>
  );
});

function DriveBar(): JSX.Element {
  const { t } = useTranslation();
  const { info, drive, lang, scan } = useApp(useShallow((s) => ({ info: s.info, drive: s.drive, lang: s.lang, scan: s.scan })));
  const [q] = useQuarantine();
  const d = info?.drives.find((x) => x.drive === drive);
  if (!d) return <footer className="drivebar" />;
  const notFreed = q?.space.find(([k]) => k === drive)?.[1].notFreedBytes ?? 0;
  const used = d.totalBytes - d.freeBytes;
  const pct = (n: number): string => `${String((n / (d.totalBytes || 1)) * 100)}%`;
  return (
    <footer className="drivebar" data-generation={scan?.generation}>
      <div className="track">
        <i className="used" style={{ width: pct(Math.max(0, used - notFreed)) }} />
        <i className="q" style={{ width: pct(notFreed) }} />
        <i className="free" style={{ width: pct(d.freeBytes) }} />
      </div>
      <div className="track-legend">
        <span>
          <i className="k" style={{ background: "var(--accent)" }} />
          {t("drive.used", { size: fmtBytes(used, lang) })}
        </span>
        <span>
          <i className="k" style={{ background: "var(--free)" }} />
          {t("drive.free", { size: fmtBytes(d.freeBytes, lang) })}
        </span>
        <span>
          <i className="k" style={{ background: "var(--q)" }} />
          {t("drive.notFreed", { size: fmtBytes(notFreed, lang) })}
        </span>
        <span className="total">{t("drive.of", { size: fmtBytes(d.totalBytes, lang) })}</span>
      </div>
    </footer>
  );
}

function FirstRun(): JSX.Element {
  const { t } = useTranslation();
  const { info, updateSettings } = useApp(useShallow((s) => ({ info: s.info, updateSettings: s.updateSettings })));
  const system = info?.drives.find((d) => d.system)?.drive ?? "C:";
  return (
    <div className="overlay" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
      <div className="welcome">
        <div style={{ width: 96, height: 96 }}>
          <Lumen />
        </div>
        <h1 id="welcome-title">{t("firstRun.title")}</h1>
        <p className="info">{t("firstRun.body")}</p>
        <p className="note">{t("firstRun.privacy")}</p>
        <div className="btns">
          <button
            className="btn primary"
            autoFocus
            onClick={() => {
              updateSettings({ firstRunDone: true });
              useApp.getState().setDrive(system);
              startScan(system, t);
            }}
          >
            {t("firstRun.start", { drive: system })}
          </button>
        </div>
      </div>
    </div>
  );
}

/** The app. */
export function App(): JSX.Element {
  const { t } = useTranslation();
  const { screen, setScreen, settings, info, drive, setDrive, scan, lang, query, setQuery, updateSettings, growthMap, setGrowthMap } = useApp(
    useShallow((s) => ({
      screen: s.screen,
      setScreen: s.setScreen,
      settings: s.settings,
      info: s.info,
      drive: s.drive,
      setDrive: s.setDrive,
      scan: s.scan,
      lang: s.lang,
      query: s.query,
      setQuery: s.setQuery,
      updateSettings: s.updateSettings, growthMap: s.growthMap, setGrowthMap: s.setGrowthMap,
    })),
  );
  const scanning = scan ? !scan.done : false;

  useEffect(() => {
    const un = onScanProgress((s) => {
      const app = useApp.getState();
      const was = app.scan;
      app.setScan(s);
      if (!s.done && Math.floor(s.elapsedMs / PROGRESS_BUBBLE_MS) !== Math.floor((was?.elapsedMs ?? 0) / PROGRESS_BUBBLE_MS)) {
        app.say({ type: "progress", text: t("companion.script.scanProgress.0", { files: fmtInt(s.files, app.lang) }), key: "scan" });
      }
      if (s.done && was && !was.done) {
        api
          .view(s.generation, 0, 3, app.settings.measure === "logical")
          .then((v) => {
            const top = v.view.children.map((c) => `${c.name} (${fmtBytes(c.onDisk, app.lang)})`).join(", ");
            app.endTopic("scan");
            app.say({ type: "explanation", text: t("companion.script.scanDone.0", { top }) });
            if (useApp.getState().adminArmed) readProtected(s.generation, t);
            tellGrowth(app.drive, t);
            if (v.hiddenBytes) app.say({ type: "tip", text: t("companion.script.hiddenSpace.0", { size: fmtBytes(v.hiddenBytes, app.lang) }) });
          })
          .catch(() => {
            app.endTopic("scan");
          });
      }
    });
    return () => {
      un.then((f) => {
        f();
      }).catch(() => undefined);
    };
  }, [t]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.target instanceof HTMLInputElement) return;
      const app = useApp.getState();
      if (e.key === "Backspace" && app.screen === "overview" && app.scan) {
        e.preventDefault();
        api
          .view(app.scan.generation, app.focus, 1, false)
          .then((v) => {
            const parent = v.view.breadcrumb[v.view.breadcrumb.length - 2];
            if (parent) app.setFocus(parent.id);
          })
          .catch(() => undefined);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  useEffect(() => {
    measureSince("unhoard:screen-paint", "unhoard:screen-switch");
  }, [screen]);

  return (
    <div className={`app${settings.motion === "calm" ? " calm" : ""}`}>
      <nav className="nav" aria-label={t("app.name")}>
        <div className="brand">
          <Lumen />
          {t("app.name")}
        </div>
        {SCREENS.map((s) => (
          <button
            key={s}
            aria-current={screen === s ? "page" : undefined}
            onClick={() => {
              setScreen(s);
            }}
          >
            {ICONS[s]}
            {t(`nav.${s}`)}
          </button>
        ))}
        <div className="nav-foot">{t("settings.privacy")}</div>
      </nav>
      <header className="top">
        <select
          value={drive}
          onChange={(e) => {
            setDrive(e.target.value);
          }}
          aria-label={t("top.scan")}
        >
          {(info?.drives ?? []).map((d) => (
            <option key={d.drive} value={d.drive}>
              {t("top.drive", { drive: d.drive, size: fmtBytes(d.totalBytes, lang) })}
              {d.system ? ` · ${t("top.systemDrive")}` : ""}
            </option>
          ))}
        </select>
        {scanning ? (
          <button
            className="btn"
            onClick={() => {
              api.scanCancel().catch(() => undefined);
            }}
          >
            {t("top.cancel")}
          </button>
        ) : (
          <button
            className="btn primary"
            onClick={() => {
              startScan(drive, t);
            }}
          >
            {scan ? t("top.rescan") : t("top.scan")}
          </button>
        )}
        {/* Search and the map style act on the map, so they show only with the overview. */}
        {screen === "overview" ? (
          <>
            <input
              value={query}
              placeholder={t("top.search")}
              aria-label={t("top.search")}
              onChange={(e) => {
                setQuery(e.target.value);
              }}
            />
            <div className="seg" role="group">
              <button
                aria-pressed={settings.mapStyle === "cells"}
                onClick={() => {
                  updateSettings({ mapStyle: "cells" });
                }}
              >
                {t("top.cells")}
              </button>
              <button
                aria-pressed={settings.mapStyle === "treemap"}
                onClick={() => {
                  updateSettings({ mapStyle: "treemap" });
                }}
              >
                {t("top.treemap")}
              </button>
              <button
                aria-pressed={settings.mapStyle === "matrix"}
                onClick={() => {
                  updateSettings({ mapStyle: "matrix" });
                }}
              >
                {t("top.matrix")}
              </button>
            </div>
            {settings.mapStyle !== "matrix" ? (
              <button
                className="btn small"
                aria-pressed={growthMap}
                title={t("top.growthHint")}
                onClick={() => {
                  setGrowthMap(!growthMap);
                }}
              >
                {t("top.growth")}
              </button>
            ) : null}
          </>
        ) : null}
        <span className="status">
          {scan
            ? t(scan.done ? "top.scanDone" : "top.scanning", { files: fmtInt(scan.files, lang), seconds: (scan.elapsedMs / 1000).toFixed(1) })
            : t("top.ready")}
        </span>
      </header>
      <main className="main">
        {/* The overview stays mounted, so switching back never rebuilds the map. */}
        <Overview active={screen === "overview"} />
        {screen === "suggestions" ? <Suggestions /> : null}
        {screen === "history" ? <History /> : null}
        {screen === "optimise" ? <Optimise /> : null}
        {screen === "quarantine" ? <Quarantine /> : null}
        {screen === "cloud" ? <Cloud /> : null}
        {screen === "photos" ? <Photos /> : null}
        {screen === "settings" ? <Settings /> : null}
      </main>
      <ChatPanel />
      <DriveBar />
      {!settings.firstRunDone && info ? <FirstRun /> : null}
    </div>
  );
}
