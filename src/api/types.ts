// TypeScript mirrors of the Rust types the commands return (serde camelCase).

export type Category = "media" | "apps" | "downloads" | "system" | "documents" | "developer" | "cloud" | "hidden" | "other";
export type Tier = "safe" | "likelySafe" | "review" | "keep";
export type NodeKind = "file" | "folder" | "link";
export type PinState = "onlineOnly" | "locallyAvailable" | "alwaysKeep";
export type WindowsTool = "storageSettings" | "storageSense" | "diskCleanup" | "apps" | "systemProtection" | "recycleBin" | "taskManager" | "windowsUpdate" | "disks";
export type Action =
  | { type: "quarantine" }
  | { type: "freeUpSpace" }
  | { type: "windowsTool"; tool: WindowsTool }
  | { type: "uninstaller" }
  | { type: "appUninstall" }
  | { type: "showInExplorer" }
  | { type: "explainOnly" };

export type FsError =
  | { kind: "notFound" | "accessDenied" | "locked" | "diskFull" | "alreadyExists" | "changed" }
  | { kind: "unsupported"; reason: string }
  | { kind: "other"; code: number };

export type CmdError =
  | { code: "noScan" | "staleScan" | "busy" }
  | { code: "unknownItem"; id: number }
  | { code: "invalidArgument"; what: string }
  | { code: "fs"; error: FsError };

export type Place =
  | "downloads"
  | "desktop"
  | "documents"
  | "pictures"
  | "videos"
  | "music"
  | "cloud"
  | "appData"
  | "userPrograms"
  | "storeApp"
  | "temp"
  | "programFiles"
  | "programData"
  | "windows"
  | "profile"
  | "otherUser"
  | "recycleBin"
  | "quarantine"
  | "drive";

export type FileType =
  | "video"
  | "photo"
  | "rawPhoto"
  | "audio"
  | "archive"
  | "diskImage"
  | "virtualDisk"
  | "installer"
  | "program"
  | "library"
  | "document"
  | "spreadsheet"
  | "presentation"
  | "pdf"
  | "ebook"
  | "text"
  | "code"
  | "data"
  | "database"
  | "log"
  | "backup"
  | "dump"
  | "aiModel"
  | "gameData"
  | "font"
  | "mail"
  | "design"
  | "partial"
  | "temporary"
  | "shortcut"
  | "system";

/** The facts an explanation is built from. */
export interface Explain {
  place: Place;
  /** The app, program or user the place belongs to. */
  owner: string | null;
  /** True when the item is the owner's own folder. */
  ownerRoot: boolean;
  fileType: FileType | null;
  /** What a folder mostly holds, when one kind fills at least half of it. */
  mix: { category: Category; percent: number } | null;
}

export interface ViewNode {
  id: number;
  name: string;
  kind: NodeKind;
  logical: number;
  onDisk: number;
  files: number;
  category: Category;
  tier: Tier | null;
  ruleId: string | null;
  explain: Explain;
  hasChildren: boolean;
  unscanned: boolean;
  cloud: PinState | null;
}

export interface Crumb {
  id: number;
  name: string;
}

export interface Others {
  count: number;
  logical: number;
  onDisk: number;
}

export interface View {
  node: ViewNode;
  path: string;
  breadcrumb: Crumb[];
  children: ViewNode[];
  others: Others;
}

export interface ViewResponse {
  generation: number;
  view: View;
  hiddenBytes: number | null;
  scanning: boolean;
}

export type ProtectReason =
  | "windows"
  | "programFiles"
  | "programData"
  | "usersFolder"
  | "profileRoot"
  | "driveRoot"
  | "systemVolumeInformation"
  | "recycleBin"
  | "quarantine";

export interface Description {
  ruleId: string | null;
  tier: Tier | null;
  actions: Action[];
  /** A large item no rule has an opinion about. */
  large: boolean;
  /** The rule is a definition: it explains the item, without an opinion. */
  definition: boolean;
  explain: Explain;
  protected: ProtectReason | null;
  cloud: boolean;
  quarantineRoot: string | null;
  requiresAdmin: boolean;
  program: Program | null;
  steam: SteamInfo | null;
  elevated: boolean;
  lastUse: LastUse | null;
  leftover: Leftover | null;
}

export type UseSource = "played" | "started" | "changed";

export interface LastUse {
  /** Seconds since 1970. */
  at: number;
  source: UseSource;
}

export interface Program {
  name: string;
  publisher: string | null;
  installLocation: string;
  /** YYYYMMDD */
  installDate: string | null;
  estimatedBytes: number | null;
}

export type SteamInfo = { kind: "game"; appId: number; name: string; lastPlayed: number | null } | { kind: "leftover" };

export interface MatrixPoint {
  id: number;
  name: string;
  label: string | null;
  ruleId: string | null;
  onDisk: number;
  lastUse: LastUse | null;
  category: Category;
  tier: Tier;
}

export interface OllamaModel {
  name: string;
  manifest: string;
  blobs: string[];
  uniqueBlobs: string[];
  bytes: number;
  uniqueBytes: number;
  /** Seconds since 1970. */
  modified: number;
  /** Scan IDs of the manifest and the blobs only this model uses; null if one is missing. */
  ids: number[] | null;
}

export interface DockerUsage {
  kind: string;
  total: number;
  active: number;
  bytes: number;
  reclaimable: number;
}

export type ToolError = { kind: "notInstalled" | "notRunning" | "timedOut" | "failed" };
export type AppName = "dockerDesktop" | "ollama" | "steam" | "ollamaDownload";
export type OpenAppResult = { Ok: null } | { Err: ToolError };
export type DockerResult = { Ok: DockerUsage[] } | { Err: ToolError };

export interface MeasuredSummary {
  folders: number;
  onDisk: number;
}
export type AdminReadResult = { Ok: MeasuredSummary } | { Err: RevealError };
export type AdminStartResult = { Ok: boolean } | { Err: RevealError };

export interface CloudSummary {
  cloudBytes: number;
  localBytes: number;
  onlineOnly: number;
  locallyAvailable: number;
  alwaysKeep: number;
}

export interface Details {
  node: ViewNode;
  path: string;
  description: Description;
  cloud: CloudSummary | null;
  /** For a folder read with admin rights: its largest children. */
  measured: [string, number][] | null;
}

export interface Suggestion {
  node: ViewNode;
  path: string;
  tier: Tier;
  description: Description;
}

export type SkipReason =
  | { kind: "protected"; detail: ProtectReason }
  | { kind: "changed" | "link" | "unsupportedName" | "insideSelection" }
  | { kind: "unavailable"; detail: FsError };

export interface MovePlan {
  moves: { node: number; path: string; destRoot: string; logical: number; onDisk: number; cloud: boolean }[];
  skipped: { path: string; reason: SkipReason }[];
  totalLogical: number;
  totalOnDisk: number;
  destinations: string[];
}

export type Outcome =
  | { outcome: "moved"; to: string }
  | { outcome: "restored"; to: string; renamed: boolean }
  | ({ outcome: "failed" } & FsError);

export interface ItemResult {
  path: string;
  outcome: Outcome;
}

export interface MoveReport {
  batchDirs: string[];
  results: ItemResult[];
  movedOnDisk: number;
}

export type ItemStatus = "moving" | "quarantined" | "restored" | "deletedByUser" | "purged";

export interface ManifestItem {
  id: string;
  originalPath: string;
  quarantinePath: string;
  kind: NodeKind;
  logicalBytes: number;
  onDiskBytes: number;
  fileCount: number;
  ruleId: string | null;
  tier: Tier | null;
  cloud: boolean;
  status: ItemStatus;
}

export interface Manifest {
  batchId: string;
  createdAt: string;
  createdUnix: number;
  purgeAfter: string;
  purgeAfterUnix: number;
  items: ManifestItem[];
}

export interface Warning {
  batchId: string;
  dir: string;
  daysLeft: 1 | 3 | 5;
  onDiskBytes: number;
  purgeAfterUnix: number;
}

export interface DriveSpace {
  freeBytes: number;
  notFreedBytes: number;
}

export interface QuarantineState {
  batches: { dir: string; manifest: Manifest }[];
  warnings: Warning[];
  space: [string, DriveSpace][];
}

export type FreeUpSkip = { kind: "notInSync" | "notInSyncRoot" } | { kind: "unavailable"; detail: FsError };

export interface FreeUpPlan {
  files: [number, string][];
  bytesToFree: number;
  skipped: [string, FreeUpSkip][];
}

export interface FreeUpResult {
  failed: [string, FsError][];
  requestedBytes: number;
  stillLocalBytes: number;
}

export interface RevealedFolder {
  path: string;
  logical: number;
  onDisk: number;
  files: number;
  children: [string, number][];
  error: FsError | null;
}

export type RevealError = { kind: "declined" } | { kind: "invalidFolder"; detail: string } | { kind: "failed"; detail: FsError };
export type HiddenKind = "restorePoints" | "storeApps" | "recovery" | "defender" | "errorReports" | "otherAccounts" | "recycleBin" | "other";
export interface RevealedView extends RevealedFolder {
  kind: HiddenKind;
}
export type RevealResult = { Ok: RevealedView[] } | { Err: RevealError };

export type Source = "whatsApp" | "samsung" | "pixel" | "iPhone" | "windowsPhone" | "screenshot" | "unknown";

export interface MediaItem {
  node: number;
  path: string;
  name: string;
  ext: string;
  size: number;
  modified: number;
  cloudOnly: boolean;
  inSyncRoot: boolean;
  parsed: { source: Source; taken: number | null; precise: boolean; video: boolean };
}

export type GroupKind = "exactDuplicates" | "nearDuplicates" | "burst" | "chatMedia" | "screenshots";
export type Certainty = "identical" | "almostCertainlyIdentical" | "likelyIdentical" | "verySimilar" | "sameMoment" | "sameSource";
export type KeeperReason = "sharpest" | "highestResolution" | "largestFile";

export interface Group {
  kind: GroupKind;
  certainty: Certainty;
  items: number[];
  label: string;
  keeper: number | null;
  keeperReasons: KeeperReason[];
}

export interface PhotoState {
  generation: number;
  media: MediaItem[];
  groups: Group[];
}

export interface ThumbOut {
  width: number;
  height: number;
  rgba: string;
}

export interface DriveInfo {
  drive: string;
  totalBytes: number;
  freeBytes: number;
  fileSystem: string;
  system: boolean;
}

export interface AppInfo {
  drives: DriveInfo[];
  syncRoots: string[];
  build: string;
}

export interface Settings {
  language: "en" | "nl" | null;
  mapStyle: "cells" | "treemap" | "matrix";
  motion: "living" | "calm";
  measure: "onDisk" | "logical";
  companion: "normal" | "quiet";
  adminRead: "ask" | "always" | "never";
  firstRunDone: boolean;
  /** Background insights: the daily measurement for storage history. */
  insights: boolean;
  /** The AI chat through Ollama on this PC. */
  chat: boolean;
  /** The chat model, or null for the recommended one. */
  chatModel: string | null;
}

/** Why a folder is a leftover of an uninstalled app. */
export type Leftover = { kind: "goneApp"; name: string; lastRun: number } | { kind: "unclaimed"; unchangedDays: number };

export type SnapshotKind = "scan" | "daily";

export interface Totals {
  used: number | null;
  free: number | null;
  hidden: number | null;
}

export interface TimelinePoint {
  /** Seconds since 1970. */
  at: number;
  kind: SnapshotKind;
  totals: Totals;
}

export interface GrowthItem {
  path: string;
  name: string;
  before: number;
  after: number;
  delta: number;
  id: number | null;
  ruleId: string | null;
  explain: Explain;
  /** [seconds since 1970, size on disk] per snapshot of the period. */
  trend: [number, number][];
}

export interface GrowthView {
  from: number | null;
  to: number | null;
  items: GrowthItem[];
  deltas: [string, number][];
}

export type FindingGroup = "performance" | "storage" | "startup" | "space" | "health" | "config";
export type FindingState = "ok" | "look" | "action";

export interface Finding {
  id: string;
  group: FindingGroup;
  state: FindingState;
  values: [string, string][];
  source: string;
}

export type StartupScope = "userRun" | "machineRun" | "userFolder" | "commonFolder";

export interface StartupChange {
  name: string;
  scope: StartupScope;
  old: number[] | null;
  enabled: boolean;
}

export type StartupError = "needsAdmin" | "notFound" | "failed";
export type ChatError = "notRunning" | "refused" | "badReply" | "notAllowed" | "off";

/** Where Ollama stands on this PC. */
export type OllamaState = "notInstalled" | "notRunning" | "running";

/** The model Unhoard recommends for this PC. */
export interface Recommendation {
  model: string;
  fit: "fast" | "slow" | "tooSmall";
  download: number;
}

/** What Settings needs to set up the chat. */
export interface ChatSetup {
  ollama: OllamaState;
  models: ModelInfo[];
  recommendation: Recommendation;
  ram: number | null;
  vram: number | null;
}

/** Download progress of a model. */
export interface PullProgress {
  status: string;
  completed: number | null;
  total: number | null;
}

export interface ModelInfo {
  name: string;
  size: number;
  recommended: boolean;
}

export type ChatEvent = { kind: "delta"; text: string } | { kind: "tool"; name: string } | { kind: "done" };

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export type Result<T, E> = { Ok: T } | { Err: E };

export interface ScanStatus {
  generation: number;
  root: string;
  driveScan: boolean;
  files: number;
  onDisk: number;
  done: boolean;
  partial: boolean;
  elapsedMs: number;
}

export interface CloudRoot {
  path: string;
  id: number | null;
  summary: CloudSummary | null;
}

export interface PurgeReport {
  purged: number;
  freedOnDisk: number;
  mismatched: number;
}
