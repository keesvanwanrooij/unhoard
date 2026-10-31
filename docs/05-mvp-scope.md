[Unhoard](../README.md) › [Docs](README.md) › Product › MVP scope

# MVP scope

The first build of Unhoard, written so it can be built in one go. Every feature has
acceptance criteria written as testable statements. The [test list](#tests-that-come-first)
is written and seen to fail **before** the code that makes it pass (see
[Development](22-development.md#test-driven-development)).

Every choice behind these criteria, with its reason, is in [Decisions](24-decisions.md).
Questions that are still open are listed there under
[Open questions](24-decisions.md#open-questions).

## Scope at a glance

| # | Feature | In the MVP | In Next |
|---|---|---|---|
| F1 | Scan | Parallel scan without admin of one drive, all drives or a folder, with live build-up | MFT turbo-scan, background scans |
| F2 | Storage map | Living cell map (default) and treemap switch, synced list, colour by kind, zoom and breadcrumb | Storage history and growth timeline |
| F3 | Hidden space | Hidden-space cell with explanation and an elevated read-only "reveal" rescan | – |
| F4 | Known locations | Library with explanation, risk tier, reason and action for each entry in [Known locations](12-known-locations.md) | Leftovers of uninstalled apps |
| F5 | Quarantine | Central quarantine with manifest, restore, 30-day purge, companion warnings, links to Windows tools | Quarantine on another drive |
| F6 | OneDrive | Local versus cloud size, placeholder states, free up space | OneDrive login (Graph), other providers tested |
| F7 | Photo and video review | Duplicates, near duplicates, bursts, chat media, screenshots, best-shot suggestion; video exact duplicates; no hydration | Similar videos (ffmpeg), local AI categories and faces |
| F8 | Companion | Scripted speech bubbles, no AI | AI chat with a local model |
| F9 | Languages | English and Dutch, following the Windows language, switchable | More languages |
| F10 | CLI | `unhoard scan <path> [--json]` | More commands |
| F11 | Distribution | Per-user installer, portable exe, winget manifest (not submitted) | Code signing, Microsoft Store |
| – | Optimise | Not in the MVP | The whole check-up, see [Optimise](16-optimise.md) |

The MVP makes **no network calls at all**. The opt-in update check is in Next. The MVP
also runs nothing in the background: background insights arrive with Optimise in Next.

## Safety invariants

These hold for the whole app. Each has at least one automated test (marked T-S…).

| ID | Invariant |
|---|---|
| S1 | Unhoard never selects, moves or deletes anything without a user click. The only automatic action is the quarantine purge (F5). |
| S2 | No code path opens a cloud placeholder for reading data. Online-only files are never hydrated. |
| S3 | Cloud files are never moved into the local quarantine. |
| S4 | Protected paths are never moved or deleted: the Windows folder, Program Files folders, ProgramData internals, user profile roots, drive roots and the quarantine root itself. See [Cleanup and safety](11-cleanup-safety.md#protected-paths). |
| S5 | Every move writes its manifest entry before the move starts, so a restore is always possible. |
| S6 | Purge deletes only inside quarantine folders and never follows links out of them. |
| S7 | The main app never runs elevated. The elevated helper is read-only. |
| S8 | The MVP opens no network connections. |
| S9 | Scan results stay in memory unless the user exports them. Exports and CLI output replace the profile path with `%USERPROFILE%`. Logs contain no file paths unless the user turns on diagnostic logging. |
| S10 | Freed space is reported only after it is really free (re-measured). |

## F1 · Scan

**What:** a fast parallel scan of what the user chooses, shown live.

**Acceptance criteria**

- AC1.1 The user can scan one drive, all fixed drives, or a chosen folder. The first-run
  default is the system drive.
- AC1.2 The scan runs without admin. Folders that deny access are counted as "unscanned",
  listed with their path, and do not stop the scan.
- AC1.3 The first results reach the UI within 1 second of starting. Updates arrive at most
  every 250 ms. The UI never freezes during a scan.
- AC1.4 A file with several hard links is counted once (by file ID). Two hard links to one
  10 MB file add 10 MB, not 20 MB.
- AC1.5 Junctions and symbolic links are recorded but not followed. A junction loop does
  not hang or crash the scan.
- AC1.6 Cloud placeholder folders are traversed. Online-only files report their logical
  size and 0 bytes on disk. No file is hydrated (S2).
- AC1.7 Each entry records logical size and size on disk. Totals use size on disk by
  default; the user can switch to logical size.
- AC1.8 Paths longer than 260 characters are scanned.
- AC1.9 Cancel stops the scan within 1 second and keeps the partial result, labelled
  "partial".
- AC1.10 Rescanning a folder replaces only that subtree.

**Out of scope:** MFT reading, scanning network shares, scheduled scans.

## F2 · Storage map

**What:** the living cell map with a treemap switch, synced with a list. Details in
[Storage map](10-storage-map.md).

**Acceptance criteria**

- AC2.1 The cell map is the default view. The switch to the treemap is remembered.
- AC2.2 After the layout settles, every cell's area is within 3% of its share of the
  parent area (tested on generated data, cells map and treemap).
- AC2.3 Cells are coloured by kind of content: photos and videos, apps and games,
  downloads, system, documents, developer, cloud, plus hidden space and other. A legend
  is always available, and colour is never the only signal (labels, patterns).
- AC2.4 Clicking a cell zooms into it. A breadcrumb shows the path. The breadcrumb and
  Backspace go up a level.
- AC2.5 A list next to the map shows the same children sorted by size. Hover and
  selection are synced between list and map.
- AC2.6 Items smaller than 0.5% of the current view are grouped into one "N smaller
  items" cell.
- AC2.7 During a scan the cells grow smoothly as sizes arrive. At rest they breathe
  subtly. With Windows animation effects off (`prefers-reduced-motion`), all ambient
  motion stops and transitions last at most 200 ms.
- AC2.8 Keyboard: arrow keys move between cells, Enter zooms in, the Menu key or
  Shift+F10 opens the actions for the selected item.
- AC2.9 The map holds 60 frames per second at rest with up to 300 visible cells on
  integrated graphics. If frames drop, visual effects are reduced first.

## F3 · Hidden space

**Acceptance criteria**

- AC3.1 For a whole-drive scan, hidden space = (total − free) − scanned size on disk.
  It shows as one hatched cell with its size.
- AC3.2 The cell's explanation lists what usually hides there and subtracts parts that
  can be measured without admin, for example the current user's Recycle Bin.
- AC3.3 "Reveal hidden space" starts the elevated read-only helper after a UAC prompt.
  It scans only the folders that were unscanned plus System Volume Information. Its
  results replace part of the hidden cell with named cells.
- AC3.4 If UAC is declined, nothing changes and the companion explains what that means.
- AC3.5 The helper exits when done and offers no command that moves, changes or deletes
  files (S7).

## F4 · Known locations

**What:** the library in [Known locations](12-known-locations.md).

**Acceptance criteria**

- AC4.1 The library is data (rule files) loaded at start. Each rule has an ID, a match,
  a category, a risk tier, an action, and text keys in English and Dutch.
- AC4.2 An item that matches a rule shows the explanation, the tier badge with its
  reason, and the action.
- AC4.3 Every rule has a generated fixture that proves it matches, plus a near miss that
  proves it does not over-match.
- AC4.4 A rule without text in both languages fails CI.
- AC4.5 Every entry marked MVP in [Known locations](12-known-locations.md) exists.
- AC4.6 Supported actions: move to quarantine, free up space (cloud), open a Windows tool
  (Storage settings, Storage Sense, Disk Cleanup, Apps), open an app's uninstaller or
  launcher, show in Explorer, explain only. There is no "delete now" action: permanent
  deletion happens only in the purge (F5).
- AC4.7 A "Suggestions" view lists all matched items by size, grouped by tier. Nothing in
  it is pre-selected (S1).
- AC4.8 Items under a protected path never offer a move (S4).
- AC4.9 When several rules match one item, the most specific rule wins (the longest
  matched path). On a tie, the strictest tier wins (Keep over Review over Likely safe
  over Safe), so a "Keep" item is never shown as safer.
- AC4.10 A large item that matches no rule still gets an explanation built from its
  category, extension, size and last-modified date (for example "Video file, 12 GB, not
  changed for 3 years"). Its tier is always Review and it has no suggested action beyond
  "Show in Explorer" and "Move to quarantine".

## F5 · Quarantine

**What:** the central quarantine in [Cleanup and safety](11-cleanup-safety.md#the-quarantine).

**Acceptance criteria**

- AC5.1 "Move to quarantine" first shows a preview from a dry run: the items, the total
  size, the destination, and skipped items with a reason. The dry run writes nothing.
- AC5.2 After the user confirms, each item is moved into the quarantine folder on the
  same drive. It is a rename, never a copy.
- AC5.3 Right before each move, Unhoard opens the item without following reparse points
  and checks its identity against the scan: volume serial, 128-bit file ID, size and
  last-write time. The rename goes through that same handle. On a mismatch the item is
  skipped and reported.
- AC5.4 Before a move, its manifest entry (original path, file ID, size, timestamps,
  operation ID and time) is written with status `moving` and flushed. If the manifest
  cannot be written, for example on a full disk, the move does not start. After the move
  the entry becomes `quarantined`. At start-up, entries left in `moving` are reconciled
  with the disk.
- AC5.5 Restore moves items back to their original paths and recreates missing parent
  folders. Restore never overwrites: if the original path exists, the item is restored
  next to it as `name (restored).ext` and the user is told.
- AC5.6 After a restore, names, timestamps and attributes match what they were before the
  move.
- AC5.7 The quarantine view shows "Not freed yet: X GB" for items on the same drive. The
  drive's free space is never shown as increased by a quarantine move (S10).
- AC5.8 Batches older than 30 days are permanently deleted, but only after the companion
  has shown the warning for that batch at least 3 days earlier. Otherwise the purge
  waits.
- AC5.9 The purge works only inside the quarantine root and only on items whose manifest
  entry still matches the file on disk. A link inside the quarantine that points outside
  it is removed as a link; its target is untouched (S6).
- AC5.10 Items the user deleted in Explorer are detected and shown as "deleted by you".
- AC5.11 The app has no "delete now" button. "Open in Explorer" opens the quarantine
  folder, and the companion explains how to delete early (Shift+Delete, or Delete and
  then empty the Recycle Bin).
- AC5.12 Protected paths and cloud placeholders are refused with a reason (S3, S4).
- AC5.13 Partial failure is reported per item: a locked file, access denied or a file
  that vanished fails alone with its reason, and the other items continue.
- AC5.14 Every move, restore and purge appends to the local audit log: the operation,
  item IDs and result, never file contents.
- AC5.15 Tauri commands that move, restore or free up space take scan-index item IDs,
  never path strings from the UI. Rust resolves each ID and re-checks the item (AC5.3).

## F6 · OneDrive

**What:** cloud awareness through the local sync folder. Details in
[Cloud storage](13-cloud-storage.md).

**Acceptance criteria**

- AC6.1 Sync roots on the PC are detected. OneDrive personal is tested; other providers
  show as "detected, not tested".
- AC6.2 Every folder in a sync root shows its cloud size (logical) and local size (on
  disk), plus counts of online-only, locally available and always-keep files.
- AC6.3 "Free up space" on a file or folder first shows a preview: the file count, the
  local size to be freed, and skipped files with a reason. After the user confirms, it
  changes only the pin state: it clears "always keep" and makes the items online-only.
  After the provider frees the data, Unhoard re-measures and reports the space freed
  (S10).
- AC6.4 Files that are not yet uploaded (not in sync) are not offered for "free up
  space".
- AC6.5 No operation reads file data inside a sync root (S2). A test with the fake
  platform layer fails if any path would do so.
- AC6.6 Cloud items never offer the local quarantine (S3). They offer the cloud
  quarantine instead: an `Unhoard Quarantine` folder inside the same sync root.
- AC6.7 A move into the cloud quarantine has the same preview, identity check, manifest
  and per-item report as F5. It is a rename inside the sync root that opens no file data,
  so OneDrive syncs it as a move without a download. After 30 days the purge deletes the
  batch inside the sync root, which sends it to the provider's recycle bin. Details in
  [Cloud storage](13-cloud-storage.md#removing-cloud-items).

## F7 · Photo and video review

**What:** grouped review of photos and videos. Details in [Photo review](14-photo-review.md).

**Acceptance criteria**

- AC7.1 A media index is built from the scan (images and videos by extension), with the
  source (phone brand, chat app, screenshot) detected from file-name patterns.
- AC7.2 Groups are formed for: exact duplicates, near duplicates, bursts, chat media and
  screenshots. Each group shows how sure the match is.
- AC7.3 Exact duplicates of local files are confirmed by a full content hash. Cloud-only
  files that match on size, extension and thumbnail hash are labelled "almost certainly
  identical", never "identical".
- AC7.4 Online-only files are never hydrated (S2). If a thumbnail cannot be obtained
  without hydration, the item is grouped by name, size and date only, and marked so.
- AC7.5 The review grid starts with nothing selected. "Select all but the suggested
  keeper" is one user action (S1). Keyboard: arrows move, Space toggles, Enter confirms
  the group, N skips to the next group.
- AC7.6 The best-shot suggestion is shown with a star and its reasons (sharpness,
  resolution, size).
- AC7.7 Videos: exact duplicates by size, duration and resolution, read from the Windows
  property system.
- AC7.8 Local items go to the quarantine, with the F5 preview. Cloud items go to the
  cloud quarantine (AC6.7).

## F8 · Companion

**What:** an original character with scripted speech bubbles. Details in
[Companion](15-companion.md).

**Acceptance criteria**

- AC8.1 Bubbles are never modal. They never block clicks on the app and Esc dismisses
  them.
- AC8.2 Every trigger in the companion script has text in English and Dutch (test).
- AC8.3 One bubble shows at a time. A priority queue puts warnings (purge soon) above tips.
- AC8.4 Settings offer "quiet mode" (only warnings). Reduced motion is respected.
- AC8.5 A history panel lists earlier bubbles, so nothing important is lost.
- AC8.6 Purge warnings show 5, 3 and 1 day(s) before a batch is purged.

## F9 · Languages

**Acceptance criteria**

- AC9.1 The interface follows the Windows display language: Dutch for `nl-*`, English
  otherwise. A choice in settings overrides it and is remembered.
- AC9.2 No UI text is hard-coded (lint rule). English and Dutch have exactly the same
  keys (test).
- AC9.3 Sizes and dates use the locale's format ("1,5 GB" in Dutch). Units are binary
  and labelled the way Explorer labels them: 1 GB = 1024³ bytes.

## F10 · CLI

**Acceptance criteria**

- AC10.1 `unhoard scan <path>` prints a table of the largest items. `--json` prints the
  JSON below to stdout; progress goes to stderr.
- AC10.2 Options: `--top <n>` (default 20), `--depth <n>` (default 1), `--logical`
  (sort by logical size).
- AC10.3 Exit codes: 0 success (unscanned folders are listed in the output, not an
  error), 1 failure, 2 invalid arguments.
- AC10.4 The CLI uses the same core as the app: for the same folder it reports the same
  totals (test).
- AC10.5 The CLI never writes to disk and never opens a network connection.
- AC10.6 Paths in the output replace the current user's profile path with
  `%USERPROFILE%` (S9, test).
- AC10.7 A size that could not be read is `null` ("unknown"), never 0.

JSON shape (field names are part of the contract):

```json
{
  "schemaVersion": 1,
  "root": "%USERPROFILE%",
  "partial": false,
  "durationMs": 8123,
  "volume": { "totalBytes": 0, "freeBytes": 0, "fileSystem": "NTFS" },
  "totals": { "logicalBytes": 0, "onDiskBytes": 0, "files": 0, "folders": 0 },
  "hiddenBytes": null,
  "unscanned": [{ "path": "%USERPROFILE%\\AppData\\Local\\Locked", "reason": "accessDenied" }],
  "items": [
    {
      "path": "%USERPROFILE%\\Downloads",
      "kind": "folder",
      "logicalBytes": 0,
      "onDiskBytes": 0,
      "files": 0,
      "category": "downloads",
      "knownLocation": { "id": "downloads-old-installers", "tier": "likelySafe" },
      "cloud": null,
      "children": []
    }
  ]
}
```

## F11 · Distribution

**Acceptance criteria**

- AC11.1 The build produces a per-user installer that needs no admin, and a portable exe.
- AC11.2 The installer installs WebView2 if it is missing. The portable exe shows a clear
  message if WebView2 is missing.
- AC11.3 A winget manifest (per-user scope) is prepared in `packaging/winget/` and passes
  `winget validate`. It is not submitted without the maintainer's explicit go.
- AC11.4 A release workflow builds the installer, the portable exe and SHA-256
  checksums. It runs only when started by hand. Publishing needs the maintainer.
- AC11.5 Settings › About shows the build identifier for bug reports. Docs and the UI
  show no version numbers elsewhere.

## Screens

| Screen | Contents |
|---|---|
| First run | Short welcome by the companion, choose what to scan (★ system drive), privacy promise in one line |
| Overview | Stage with the map (cells or treemap), breadcrumb, synced list, details panel for the selection, drive bar (used, free, not-yet-freed quarantine) |
| Suggestions | Known-location matches by tier and size, each with explanation and action |
| Quarantine | Batches with purge date, "not freed yet" total, restore, open in Explorer |
| Cloud | Sync roots with local versus cloud size, placeholder states, free up space |
| Photos | Groups by type, review grid, best-shot star |
| Settings | Language, map style, motion (living or calm), size measure, units, companion (normal or quiet), About |

## Non-functional requirements

| Area | Requirement (targets, measured during the build) |
|---|---|
| Scan speed | A full scan of a system drive with about 1 million files on a mid-range NVMe SSD, without admin, finishes in under 60 seconds. Beating MFT-based tools is not a goal for the MVP |
| Memory | Peak memory below 400 MB for 1 million entries |
| Start-up | The window shows within 2 seconds on a mid-range PC |
| Frame rate | See AC2.9 |
| Responsiveness | The budgets P1–P14 in [Performance](25-performance.md): for example a visible response to every input within 50 ms, a screen switch painted within 100 ms, and no UI-thread task over 50 ms |
| Size | Installer below 20 MB |
| Accessibility | Every action is reachable by keyboard; the list is the accessible equivalent of the map; contrast meets WCAG AA in the shell; screen-reader labels on all controls |
| Motion | Follows the Windows animation setting; a "calm" motion setting exists regardless |
| Privacy | S8 and S9, see [Privacy](30-privacy.md) |
| Portability | All platform calls sit behind the platform layer ([Architecture](20-architecture.md)) |

## Tests that come first

Write these tests first, run them, and see them fail before writing the code. The full
names live in the code and carry the F or S ID (`s5_manifest_written_before_move`); this
list is the contract. "Fake" means the fake platform layer that simulates attributes,
placeholders and errors without a real disk or cloud.

| ID | Test | Layer |
|---|---|---|
| T-1.1 | sizes aggregate bottom-up: a folder's size equals the sum of its children | core unit |
| T-1.2 | a file with two hard links counts once | core unit (fake) + Windows integration (temp dir) |
| T-1.3 | junctions are recorded and not followed; a loop terminates | core unit (fake) + Windows integration |
| T-1.4 | access-denied folders are listed as unscanned and the scan continues | core unit (fake) |
| T-1.5 | an online-only file reports logical size and 0 on disk | core unit (fake) |
| T-1.6 | paths longer than 260 characters are scanned | Windows integration (temp dir) |
| T-1.7 | cancel returns a partial result within 1 second | core unit |
| T-1.8 | rescan replaces only the subtree | core unit |
| T-2.1 | cell map areas are within 3% after settling | UI unit (layout) |
| T-2.2 | treemap areas are within 3% | UI unit (layout) |
| T-2.3 | items below 0.5% are grouped into "N smaller items" | UI unit |
| T-2.4 | the layout is stable: the same input gives the same output | UI unit |
| T-3.1 | hidden space = used − scanned on disk, never negative | core unit |
| T-4.1 | every rule matches its fixture and not its near miss | core unit (generated fixtures) |
| T-4.2 | every rule has English and Dutch text | core unit / CI |
| T-4.3 | protected paths never get a move action | core unit |
| T-4.4 | with overlapping rules the most specific one wins, and on a tie the strictest tier | core unit |
| T-4.5 | an unmatched item gets the generic explanation and tier Review | core unit |
| T-5.1 | the manifest entry exists before the move starts | core unit (fake) |
| T-5.2 | a move followed by a restore leaves names, timestamps and attributes unchanged | Windows integration (temp dir) |
| T-5.3 | restore into a path that now exists creates `name (restored).ext` and overwrites nothing | core unit + Windows integration |
| T-5.4 | entries left in `moving` are reconciled at start-up | core unit (fake crash) |
| T-5.5 | purge waits until a warning was shown at least 3 days earlier | core unit (fake clock) |
| T-5.6 | purge deletes a link, not its target | Windows integration (temp dir) |
| T-5.7 | a locked file fails alone; the batch continues | Windows integration |
| T-5.8 | a quarantine move does not change the reported free space | core unit |
| T-5.9 | the dry run of a move and of the purge writes nothing (temp tree identical before and after) | Windows integration (temp dir) |
| T-5.10 | an item whose file ID, size or last-write time changed since the scan is skipped and reported | core unit (fake) + Windows integration |
| T-5.11 | the purge skips an item whose manifest entry no longer matches the file | core unit (fake) |
| T-5.12 | a failed manifest write (full disk) stops the move before it starts | core unit (fake) |
| T-5.13 | Unicode names and names with trailing dots or spaces survive a move and a restore | Windows integration (temp dir) |
| T-5.14 | a file that vanishes mid-operation is reported, and the batch continues | core unit (fake) |
| T-5.15 | every move, restore and purge appends one audit-log entry without file contents | core unit |
| T-5.16 | destructive commands reject path strings and unknown or stale item IDs | app unit |
| T-6.1 | free up space changes only the pin state and re-measures | core unit (fake) |
| T-6.2 | no code path opens a placeholder for data (hydration guard) | core unit (fake records every open) |
| T-6.3 | cloud items never offer the local quarantine | core unit |
| T-6.4 | files not in sync are not offered for free up space | core unit (fake) |
| T-6.5 | a cloud quarantine move is a rename inside the same sync root and opens no file data | core unit (fake records every open) |
| T-7.1 | file-name patterns map to the right source (table in [Photo review](14-photo-review.md)) | core unit |
| T-7.2 | local exact duplicates need equal full hashes | core unit (generated images) |
| T-7.3 | cloud matches are labelled "almost certainly identical" | core unit |
| T-7.4 | bursts group photos from the same source within the time window | core unit |
| T-7.5 | perceptual hash distance groups near duplicates and separates different images | core unit (generated images) |
| T-7.6 | the best shot prefers the sharpest image | core unit (generated blur) |
| T-7.7 | the review grid starts with nothing selected | UI unit |
| T-8.1 | every companion trigger has English and Dutch text | UI unit |
| T-8.2 | warnings outrank tips in the bubble queue | UI unit |
| T-9.1 | English and Dutch have the same keys | UI unit |
| T-9.2 | the language follows the Windows language with an English fallback | UI unit |
| T-10.1 | `scan --json` output matches the schema | CLI integration |
| T-10.2 | the CLI and the app core report the same totals | CLI integration |
| T-10.3 | CLI output and exports replace the profile path with `%USERPROFILE%` | CLI integration |
| T-S2 | scanning and classification change no file, timestamp or attribute (temp tree identical before and after) | Windows integration (temp dir) |
| T-S8 | the MVP binary has no network capability enabled (Tauri capabilities, no HTTP client in the dependency tree) | CI check |
| T-P1 | core benchmarks (view query, aggregation, subtree rescan, rule matching, search) on a generated 1-million-entry tree stay within P13–P14 | core benchmark (criterion) |
| T-P2 | layout benchmarks (relaxation step, treemap, morph, grouping) with 50 and 300 cells | UI benchmark (Vitest bench) |
| T-P3 | scripted interactions (screen switches, zoom, map toggle, hover, scroll, search) stay within P1–P12 at 4× CPU throttling | UI interaction (Playwright, fake core) |
| T-P4 | no benchmark is over its budget or more than 20% slower than `perf/baseline.json` | CI check |

All test data is generated by the tests in temp folders: files, folders, hard links,
junctions and images (with known blur or noise). Never a real disk, real photos or a real
scan. Real OneDrive behaviour is checked with the manual checklist in
[Development](22-development.md#manual-checks).

## Definition of done for the MVP build

The general Definition of done in [AGENTS.md](../AGENTS.md) applies. For the MVP build it
means:

- [ ] All tests above exist, were seen failing first, and pass.
- [ ] These checks pass: `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
      `cargo test --workspace`, `cargo deny check`, `npm run typecheck`, `npm run lint`,
      `npm test`, `npm run build`, `npm audit --audit-level=high`.
- [ ] `cargo bench -p unhoard-core`, `npm run perf` and `npm run perf:check` pass, and
      `perf/baseline.json` holds the first numbers ([Performance](25-performance.md)).
- [ ] The memory budget per scanned entry is checked on a generated tree of at least
      1 million entries, and the numbers are in the report.
- [ ] The app runs: scan of the system drive, cell map and treemap, zoom, suggestions, a
      quarantine round trip on a test folder, Dutch and English.
- [ ] The report has a security review for the High changes and a safety argument for
      the Critical ones (the purge).
- [ ] The manual OneDrive checklist is done on a real OneDrive folder with test files.
- [ ] Installer and portable exe build. The winget manifest validates.
- [ ] The Dependabot entries for Cargo and npm are switched on.
- [ ] Docs match the code, and [Decisions](24-decisions.md) holds every current decision
      with its reason.
- [ ] `git status` shows no private files.

---

## Related in this cluster

- [Roadmap](06-roadmap.md): what comes after the MVP.
- [Vision](01-vision.md): the principles these criteria protect.
- [Target users](02-target-users.md): whose needs the MVP covers.
- [Competitive advantage](04-competitive-advantage.md): which advantages the MVP makes real.

Back to the [docs hub](README.md).
