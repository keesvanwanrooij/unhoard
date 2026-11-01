// The companion's triggers. Each has one or more lines under `companion.script.<trigger>`
// in both locale files; one line is picked so the companion does not repeat itself.

/** Every trigger the app fires. */
export const TRIGGERS = [
  "firstRun",
  "scanStart",
  "scanProgress",
  "scanDone",
  "hiddenSpace",
  "revealDeclined",
  "revealDone",
  "selected.safe",
  "selected.likelySafe",
  "selected.review",
  "selected.keep",
  "selected.protected",
  "previewShown",
  "quarantineMoved",
  "purgeWarning.5",
  "purgeWarning.3",
  "purgeWarning.1",
  "deleteEarly",
  "restored",
  "freeUpSpaceDone",
  "photoReview",
  "photoNoThumbs",
  "windowsTool",
  "zoomTip",
  "cloudIntro",
  "adminAsk",
  "adminReadDone",
  "error",
] as const;

/** A trigger name. */
export type Trigger = (typeof TRIGGERS)[number];
