// F1 scan progress on the overview: scanned bytes against the drive's used space.
import { describe, expect, it } from "vitest";

import type { DriveInfo, ScanStatus } from "../api/types";
import { scanFraction } from "./ScanBar";

const GB = 2 ** 30;
const drives: DriveInfo[] = [{ drive: "C:", totalBytes: 900 * GB, freeBytes: 100 * GB, fileSystem: "NTFS", system: true }];

function status(over: Partial<ScanStatus>): ScanStatus {
  return { generation: 1, root: "C:\\", driveScan: true, files: 10, onDisk: 0, done: false, partial: false, elapsedMs: 1000, ...over };
}

describe("scan progress", () => {
  it("f1_drive_scan_progress_is_scanned_against_used_space", () => {
    expect(scanFraction(status({ onDisk: 400 * GB }), drives)).toBeCloseTo(0.5);
    expect(scanFraction(status({ onDisk: 0 }), drives)).toBe(0);
  });

  it("f1_progress_stays_below_full_until_the_scan_ends", () => {
    expect(scanFraction(status({ onDisk: 820 * GB }), drives)).toBe(0.99);
    expect(scanFraction(status({ onDisk: 700 * GB, done: true }), drives)).toBe(1);
  });

  it("f1_folder_scans_and_unknown_drives_have_no_fraction", () => {
    expect(scanFraction(status({ driveScan: false, root: "C:\\Users\\u" }), drives)).toBeNull();
    expect(scanFraction(status({ root: "D:\\" }), drives)).toBeNull();
  });
});
