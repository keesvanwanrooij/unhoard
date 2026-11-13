// Typed calls to the Rust commands and events. Every call can fail with a CmdError; the
// UI turns it into a message through `errorKey`, never into a crash.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AdminReadResult,
  AdminStartResult,
  ChatError,
  ChatEvent,
  ChatMessage,
  Finding,
  GrowthView,
  ModelInfo,
  Result,
  StartupChange,
  StartupError,
  StartupScope,
  TimelinePoint,
  AppName,
  AppInfo,
  CloudRoot,
  CmdError,
  Details,
  DockerResult,
  FreeUpPlan,
  FreeUpResult,
  ItemResult,
  MatrixPoint,
  MovePlan,
  MoveReport,
  OllamaModel,
  OpenAppResult,
  PhotoState,
  PurgeReport,
  QuarantineState,
  RevealResult,
  ScanStatus,
  Settings,
  Suggestion,
  ThumbOut,
  ViewResponse,
  WindowsTool,
} from "./types";

/** The i18n key of an error. */
export function errorKey(e: unknown): string {
  const err = e as Partial<CmdError> | undefined;
  if (err && typeof err === "object" && "code" in err && typeof err.code === "string") {
    if (err.code === "fs" && "error" in err) return `errors.fs.${(err.error as { kind: string }).kind}`;
    return `errors.${err.code}`;
  }
  return "errors.invalidArgument";
}

/** The commands. */
export const api = {
  appInfo: (): Promise<AppInfo> => invoke("app_info"),
  settingsGet: (): Promise<Settings> => invoke("settings_get"),
  settingsSet: (settings: Settings): Promise<void> => invoke("settings_set", { settings }),
  scanStart: (drive: string): Promise<number> => invoke("scan_start", { drive }),
  scanCancel: (): Promise<void> => invoke("scan_cancel"),
  scanStatus: (): Promise<ScanStatus> => invoke("scan_status"),
  view: (generation: number, id: number, top: number, logical: boolean): Promise<ViewResponse> =>
    invoke("view", { generation, id, top, logical }),
  details: (generation: number, id: number): Promise<Details> => invoke("details", { generation, id }),
  suggestions: (limit: number): Promise<[number, Suggestion[]]> => invoke("suggestions", { limit }),
  quarantinePreview: (generation: number, ids: number[]): Promise<MovePlan> => invoke("quarantine_preview", { generation, ids }),
  quarantineMove: (generation: number, ids: number[]): Promise<MoveReport> => invoke("quarantine_move", { generation, ids }),
  quarantineState: (): Promise<QuarantineState> => invoke("quarantine_state"),
  quarantineRestore: (batchId: string, itemIds: string[]): Promise<ItemResult[]> => invoke("quarantine_restore", { batchId, itemIds }),
  quarantineWarningShown: (batchId: string, daysLeft: number): Promise<void> => invoke("quarantine_warning_shown", { batchId, daysLeft }),
  quarantineOpen: (batchId: string): Promise<void> => invoke("quarantine_open", { batchId }),
  showInExplorer: (generation: number, id: number): Promise<void> => invoke("show_in_explorer", { generation, id }),
  openTool: (tool: WindowsTool): Promise<void> => invoke("open_tool", { tool }),
  freeUpPreview: (generation: number, ids: number[]): Promise<FreeUpPlan> => invoke("free_up_preview", { generation, ids }),
  freeUpApply: (generation: number, ids: number[]): Promise<FreeUpResult> => invoke("free_up_apply", { generation, ids }),
  revealHidden: (generation: number): Promise<RevealResult> => invoke("reveal_hidden", { generation }),
  photosAnalyze: (): Promise<PhotoState> => invoke("photos_analyze"),
  photoThumbnail: (generation: number, index: number, side: number): Promise<ThumbOut> =>
    invoke("photo_thumbnail", { generation, index, side }),
  cloudRoots: (generation: number): Promise<CloudRoot[]> => invoke("cloud_roots", { generation }),
  matrix: (limit: number): Promise<[number, MatrixPoint[]]> => invoke("matrix", { limit }),
  adminStart: (): Promise<AdminStartResult> => invoke("admin_start"),
  adminRead: (generation: number, ids: number[] | null): Promise<AdminReadResult> => invoke("admin_read", { generation, ids }),
  ollamaModels: (generation: number): Promise<OllamaModel[]> => invoke("ollama_models", { generation }),
  dockerUsage: (): Promise<DockerResult> => invoke("docker_usage"),
  appUninstall: (generation: number, id: number): Promise<void> => invoke("app_uninstall", { generation, id }),
  openApp: (app: AppName): Promise<OpenAppResult> => invoke("open_app", { app }),
  historyTimeline: (drive: string): Promise<TimelinePoint[]> => invoke("history_timeline", { drive }),
  historyGrowth: (drive: string, days: number): Promise<GrowthView> => invoke("history_growth", { drive, days }),
  historyClear: (): Promise<void> => invoke("history_clear"),
  optimiseCheck: (): Promise<Finding[]> => invoke("optimise_check"),
  optimiseUpdates: (): Promise<Finding[]> => invoke("optimise_updates"),
  startupPreview: (scope: StartupScope, name: string, enabled: boolean): Promise<Result<StartupChange, StartupError>> =>
    invoke("startup_preview", { scope, name, enabled }),
  startupSet: (scope: StartupScope, name: string, enabled: boolean): Promise<Result<number, StartupError>> =>
    invoke("startup_set", { scope, name, enabled }),
  startupUndo: (index: number): Promise<Result<null, StartupError>> => invoke("startup_undo", { index }),
  chatModels: (): Promise<Result<ModelInfo[], ChatError>> => invoke("chat_models"),
  chatSend: (messages: ChatMessage[], lang: string): Promise<Result<null, ChatError>> => invoke("chat_send", { messages, lang }),
  chatStop: (): Promise<void> => invoke("chat_stop"),
};

/** The AI chat's answer as it forms. */
export function onChatEvent(f: (e: ChatEvent) => void): Promise<UnlistenFn> {
  return listen<ChatEvent>("chat-event", (e) => {
    f(e.payload);
  });
}

/** Live scan progress. */
export function onScanProgress(f: (s: ScanStatus) => void): Promise<UnlistenFn> {
  return listen<ScanStatus>("scan-progress", (e) => {
    f(e.payload);
  });
}

/** The result of the start-up maintenance (reconcile and purge). */
export function onMaintenanceDone(f: (r: PurgeReport) => void): Promise<UnlistenFn> {
  return listen<PurgeReport>("maintenance-done", (e) => {
    f(e.payload);
  });
}
