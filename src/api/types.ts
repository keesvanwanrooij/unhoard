// TypeScript mirrors of the Rust types the commands return (serde camelCase).

export type Category = "media" | "apps" | "downloads" | "system" | "documents" | "developer" | "cloud" | "hidden" | "other";
export type Tier = "safe" | "likelySafe" | "review" | "keep";
export type NodeKind = "file" | "folder" | "link";
export type PinState = "onlineOnly" | "locallyAvailable" | "alwaysKeep";
export type WindowsTool = "storageSettings" | "storageSense" | "diskCleanup" | "apps" | "systemProtection" | "recycleBin";
export type Action =
  | { type: "quarantine" }
  | { type: "freeUpSpace" }
  | { type: "windowsTool"; tool: WindowsTool }
  | { type: "uninstaller" }
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
  generic: boolean;
  protected: ProtectReason | null;
  cloud: boolean;
  quarantineRoot: string | null;
  requiresAdmin: boolean;
}

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
export type RevealResult = { Ok: RevealedFolder[] } | { Err: RevealError };

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
export type Certainty = "identical" | "almostCertainlyIdentical" | "verySimilar" | "sameMoment" | "sameSource";
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
  mapStyle: "cells" | "treemap";
  motion: "living" | "calm";
  measure: "onDisk" | "logical";
  companion: "normal" | "quiet";
  firstRunDone: boolean;
}

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
