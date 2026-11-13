// Builds the explanation of an item. A known-locations rule gives a written explanation;
// every other item gets sentences built from the facts the core collected: its type or
// what it holds, and the place it lies in. So each item has a description of its own.
import type { TFunction } from "i18next";

import type { Explain, FileType, Leftover, NodeKind, Place } from "./api/types";
import { fmtInt } from "./format";
import type { Lang } from "./i18n/detect";

/** Every place the core reports, for the text tests. */
export const PLACES: Place[] = [
  "downloads",
  "desktop",
  "documents",
  "pictures",
  "videos",
  "music",
  "cloud",
  "appData",
  "userPrograms",
  "storeApp",
  "temp",
  "programFiles",
  "programData",
  "windows",
  "profile",
  "otherUser",
  "recycleBin",
  "quarantine",
  "drive",
];

/** Every file type the core reports, for the text tests. */
export const FILE_TYPES: FileType[] = [
  "video",
  "photo",
  "rawPhoto",
  "audio",
  "archive",
  "diskImage",
  "virtualDisk",
  "installer",
  "program",
  "library",
  "document",
  "spreadsheet",
  "presentation",
  "pdf",
  "ebook",
  "text",
  "code",
  "data",
  "database",
  "log",
  "backup",
  "dump",
  "aiModel",
  "gameData",
  "font",
  "mail",
  "design",
  "partial",
  "temporary",
  "shortcut",
  "system",
];

/** Places whose owner has a folder of its own, described as a whole. */
const OWNED: Place[] = ["appData", "userPrograms", "storeApp", "programFiles", "programData", "otherUser"];

/** What an explanation needs to know about an item. */
export interface Explained {
  name: string;
  kind: NodeKind;
  files: number;
  ruleId: string | null;
  explain: Explain;
}

/** The extension of a file name with its dot, or "" when it has none. */
function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? name.slice(dot).toLowerCase() : "";
}

/** True when the item is the folder of the app, program or user its place belongs to. */
function isOwnerRoot(item: Explained): boolean {
  return item.kind === "folder" && item.explain.ownerRoot && item.explain.owner !== null && OWNED.includes(item.explain.place);
}

/** The sentence that says where the item lies. */
function placeSentence(e: Explain, t: TFunction): string {
  const owned = OWNED.includes(e.place) && e.owner !== null;
  return t(`explain.place.${e.place}${owned ? "Of" : ""}`, { owner: e.owner ?? "" });
}

/** The full explanation of an item: its rule's text, or sentences built from the facts. */
export function explainText(item: Explained, t: TFunction, lang: Lang): string {
  if (item.ruleId) return t(`knownLocations.${item.ruleId}.explain`);
  const e = item.explain;
  const name = item.name;
  if (isOwnerRoot(item)) return t(`explain.root.${e.place}`, { name });
  const parts: string[] = [];
  if (item.kind === "link") {
    parts.push(t("explain.link", { name }));
  } else if (item.kind === "file") {
    if (e.fileType) {
      parts.push(t("explain.file", { name, type: t(`explain.fileType.${e.fileType}.a`) }));
      parts.push(t(`explain.fileType.${e.fileType}.about`));
    } else {
      const ext = extension(name);
      parts.push(ext ? t("explain.fileOther", { name, ext }) : t("explain.fileBare", { name }));
    }
  } else {
    parts.push(item.files > 0 ? t("explain.folder", { name, count: item.files, files: fmtInt(item.files, lang) }) : t("explain.folderEmpty", { name }));
    if (e.mix) parts.push(t("explain.mix", { percent: `${String(e.mix.percent)}%`, kind: t(`explain.mixKind.${e.mix.category}`) }));
  }
  parts.push(placeSentence(e, t));
  return parts.join(" ");
}

/** One line for lists and tooltips: the rule's title, or the kind of item and its place. */
export function explainShort(item: Explained, t: TFunction): string {
  if (item.ruleId) return t(`knownLocations.${item.ruleId}.title`);
  const e = item.explain;
  if (isOwnerRoot(item)) return t(`explain.rootShort.${e.place}`);
  const owned = OWNED.includes(e.place) && e.owner !== null;
  const where = t(`explain.where.${e.place}${owned ? "Of" : ""}`, { owner: e.owner ?? "" });
  let what: string;
  if (item.kind === "link") what = t("explain.short.link");
  else if (item.kind === "file") {
    const ext = extension(item.name);
    what = e.fileType ? t(`explain.fileType.${e.fileType}.name`) : ext ? t("explain.short.file", { ext }) : t("explain.short.fileBare");
  } else what = e.mix ? t("explain.short.folderMix", { kind: t(`explain.mixKind.${e.mix.category}`) }) : t("explain.short.folder");
  return `${what} ${where}`;
}

/** Why a folder is a leftover of an uninstalled app, as one sentence. */
export function leftoverText(l: Leftover, t: TFunction, lang: Lang): string {
  return l.kind === "goneApp"
    ? t("explain.leftover.goneApp", { name: l.name, date: new Date(l.lastRun * 1000).toLocaleDateString(lang) })
    : t("explain.leftover.unclaimed", { days: fmtInt(l.unchangedDays, lang) });
}
