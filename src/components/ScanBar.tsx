// The scan's progress bar on the overview. For a drive scan the progress is the scanned
// size against the drive's used space; it stays below full until the scan ends, because
// hidden space is never scanned. A folder scan has no known total, so its bar only moves.
import type { JSX } from "react";
import { useShallow } from "zustand/react/shallow";
import { useTranslation } from "react-i18next";

import type { DriveInfo, ScanStatus } from "../api/types";
import { fmtBytes, fmtShare } from "../format";
import { useApp } from "../store";

/** The highest fraction shown while the scan still runs. */
const RUNNING_MAX = 0.99;

/** Progress from 0 to 1, or `null` when the total is unknown (a folder scan). */
export function scanFraction(s: ScanStatus, drives: DriveInfo[]): number | null {
  if (s.done) return 1;
  if (!s.driveScan) return null;
  const drive = s.root.slice(0, 2).toUpperCase();
  const d = drives.find((x) => x.drive.toUpperCase() === drive);
  if (!d) return null;
  const used = d.totalBytes - d.freeBytes;
  if (used <= 0) return null;
  return Math.min(RUNNING_MAX, Math.max(0, s.onDisk / used));
}

/** The progress bar along the top of the stage, shown while a scan runs. */
export function ScanBar(): JSX.Element | null {
  const { t } = useTranslation();
  const { scan, info, lang } = useApp(useShallow((s) => ({ scan: s.scan, info: s.info, lang: s.lang })));
  if (!scan || scan.done) return null;
  const fraction = scanFraction(scan, info?.drives ?? []);
  const label =
    fraction === null
      ? t("map.progress.folder", { size: fmtBytes(scan.onDisk, lang) })
      : t("map.progress.drive", { share: fmtShare(fraction, lang), size: fmtBytes(scan.onDisk, lang) });
  return (
    <div
      className={`scanbar${fraction === null ? " unknown" : ""}`}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
    >
      <i style={fraction === null ? undefined : { width: `${String(fraction * 100)}%` }} />
      <span>{label}</span>
    </div>
  );
}
