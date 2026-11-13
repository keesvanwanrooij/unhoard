// The app's shared state: settings, scan, navigation, selection and the companion.
import { create } from "zustand";

import { api, errorKey } from "./api/ipc";
import type { AppInfo, ScanStatus, Settings } from "./api/types";
import { BubbleQueue, type BubbleInput } from "./companion/queue";
import type { Lang } from "./i18n/detect";
import { i18n } from "./i18n";
import { mark } from "./perf/marks";

/** The screens of the app. */
export type Screen = "overview" | "suggestions" | "quarantine" | "cloud" | "photos" | "settings";

/** The default settings, before the stored ones are read. */
export const DEFAULT_SETTINGS: Settings = {
  language: null,
  mapStyle: "cells",
  motion: "living",
  measure: "onDisk",
  companion: "normal",
  adminRead: "ask",
  firstRunDone: false,
};

interface State {
  lang: Lang;
  settings: Settings;
  info: AppInfo | null;
  screen: Screen;
  drive: string;
  scan: ScanStatus | null;
  focus: number;
  selected: number | null;
  /** The item under the pointer in the list, highlighted on the map like a selection. */
  hovered: number | null;
  /** The elevated helper was started for the running scan and waits for its folders. */
  adminArmed: boolean;
  /** The user's answer to reading protected folders, kept for this app session: asked once. */
  adminAnswer: "yes" | "no" | null;
  query: string;
  /** Bumped whenever the tree changed (a move), so views reload. */
  treeVersion: number;
  queue: BubbleQueue;
  /** Bumped when the companion's queue changed, so it re-renders. */
  bubbleVersion: number;
  setScreen: (s: Screen) => void;
  setFocus: (id: number) => void;
  select: (id: number | null) => void;
  hover: (id: number | null) => void;
  setAdminArmed: (armed: boolean) => void;
  setAdminAnswer: (answer: "yes" | "no" | null) => void;
  setQuery: (q: string) => void;
  setDrive: (d: string) => void;
  setScan: (s: ScanStatus) => void;
  setInfo: (i: AppInfo) => void;
  setLang: (l: Lang) => void;
  updateSettings: (patch: Partial<Settings>) => void;
  treeChanged: () => void;
  say: (b: BubbleInput) => void;
  /** The user closed the bubble; its topic stays quiet until it ends. */
  dismiss: () => void;
  /** The bubble's time is up. */
  expire: () => void;
  /** The operation behind a topic ended; its bubbles go. */
  endTopic: (key: string) => void;
  sayError: (e: unknown, t: (k: string, o?: Record<string, unknown>) => string) => void;
}

export const useApp = create<State>((set, get) => ({
  lang: "en",
  settings: DEFAULT_SETTINGS,
  info: null,
  screen: "overview",
  drive: "C:",
  scan: null,
  focus: 0,
  selected: null,
  hovered: null,
  adminArmed: false,
  adminAnswer: null,
  query: "",
  treeVersion: 0,
  queue: new BubbleQueue(),
  bubbleVersion: 0,
  setScreen: (screen) => {
    mark("unhoard:screen-switch");
    set({ screen });
  },
  setFocus: (focus) => {
    set({ focus, selected: null, hovered: null });
  },
  select: (selected) => {
    set({ selected });
  },
  hover: (hovered) => {
    set({ hovered });
  },
  setAdminArmed: (adminArmed) => {
    set({ adminArmed });
  },
  setAdminAnswer: (adminAnswer) => {
    set({ adminAnswer });
  },
  setQuery: (query) => {
    set({ query });
  },
  setDrive: (drive) => {
    set({ drive });
  },
  setScan: (scan) => {
    const prev = get().scan;
    set(prev?.generation !== scan.generation ? { scan, focus: 0, selected: null } : { scan });
  },
  setInfo: (info) => {
    set({ info });
  },
  setLang: (lang) => {
    set({ lang });
  },
  updateSettings: (patch) => {
    const settings = { ...get().settings, ...patch };
    get().queue.quiet = settings.companion === "quiet";
    set({ settings });
    api.settingsSet(settings).catch(() => {
      // The setting stays in effect for this session; the companion says it was not saved.
      get().say({ type: "warning", text: i18n.t("errors.invalidArgument") });
    });
  },
  treeChanged: () => {
    set((s) => ({ treeVersion: s.treeVersion + 1 }));
  },
  say: (b) => {
    get().queue.push(b);
    set((s) => ({ bubbleVersion: s.bubbleVersion + 1 }));
  },
  dismiss: () => {
    get().queue.dismiss();
    set((s) => ({ bubbleVersion: s.bubbleVersion + 1 }));
  },
  expire: () => {
    get().queue.expire();
    set((s) => ({ bubbleVersion: s.bubbleVersion + 1 }));
  },
  endTopic: (key) => {
    get().queue.end(key);
    set((s) => ({ bubbleVersion: s.bubbleVersion + 1 }));
  },
  sayError: (e, t) => {
    get().say({ type: "warning", text: t("companion.script.error.0", { message: t(errorKey(e)) }) });
  },
}));
