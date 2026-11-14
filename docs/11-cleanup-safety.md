[Unhoard](../README.md) › [Docs](README.md) › Features › Cleanup and safety

# Cleanup and safety

How Unhoard helps you get space back without ever losing something you wanted. The
workflow is always the same: **analyse, assess the risk, move to the quarantine, wait
30 days, then purge.** Nothing is selected, moved or deleted without your click (S1).
Acceptance criteria: F4 and F5 in [MVP scope](05-mvp-scope.md).

## Principles

- **No deletion without context.** Every item that offers an action also says what it is,
  why it is there and what happens if it goes.
- **No delete-now button.** Permanent deletion happens only in the 30-day purge. If you
  want to delete earlier, you do so yourself in Explorer; the companion explains how.
- **For space right away,** Unhoard links to the Windows tools that clean what Windows
  owns: Storage Sense, Disk Cleanup ("Clean up system files") and Settings › Apps.
- **Honest about space.** A move to the quarantine on the same drive is a rename. It is
  instant, but it frees nothing until the batch is purged or you delete it yourself. The
  quarantine view says "Not freed yet: X GB", and the drive's free space is never shown
  as increased by a move (S10).

## Risk tiers

Every known location and every large item has one tier, always shown with its reason.

| Tier | Meaning | Typical examples | Actions |
|---|---|---|---|
| **Safe** | Regenerable. The app or Windows rebuilds it when needed | Browser and shader caches, Temp files older than 24 hours | Move to quarantine; link to Storage Sense or Disk Cleanup |
| **Likely safe** | Usually unneeded, but it was yours or an app's | Old installers in Downloads, crash dumps, Windows.old | Move to quarantine; for Windows-owned items, the Windows tool |
| **Review** | Only you can judge | Large videos, ISOs, phone backups, WSL and Docker disks, unmatched large items | Show in Explorer; move to quarantine |
| **Keep (system)** | Needed by Windows or an app; removing it by hand breaks things | `pagefile.sys`, `hiberfil.sys`, WinSxS, `Windows\Installer`, `ProgramData\Package Cache` | Explain only, plus a link to the right Windows tool |

The tier comes from the [known-locations library](12-known-locations.md). When several
rules match, the most specific wins, and on a tie the strictest tier wins, so a Keep item
is never shown as safer than it is.

## Protected paths

These paths are never moved, renamed or deleted, whatever a rule or the UI asks (S4).
Items inside them can still be explained, and Windows-owned items link to the Windows
tool that cleans them.

- `%WINDIR%` (the Windows folder), including `System32`, `WinSxS`, `Installer` and
  `DriverStore`
- `Program Files` and `Program Files (x86)`
- ProgramData internals: `ProgramData\Microsoft` and `ProgramData\Package Cache`
- `C:\Users` and every user profile root (`C:\Users\<name>` itself, not its contents)
- every drive root (`C:\`, `D:\` …)
- `System Volume Information` and `$Recycle.Bin`
- the quarantine roots themselves, local and in the cloud

Protection is checked in the Rust core when an item is resolved from its ID, not in the
UI. A path is compared after normalising it to its full `\\?\` form and resolving its
case, and a protected path is refused with a reason (AC5.12).

## The quarantine

### Where it lives

- **One quarantine folder per drive:** `X:\Unhoard Quarantine\`. A move is therefore
  always a rename on the same volume: instant, with no copy and no extra space.
- The app shows all drives' quarantines as **one central view**, so you never hunt for
  files across the PC.
- The folder name is `Unhoard Quarantine` in every language, so paths and manifests stay
  valid when the user switches language. The read-me file inside is in the user's
  language.
- On creation, Unhoard gives the folder an explicit ACL that is not inherited from the
  drive root: full control for the current user, SYSTEM and Administrators only. Without
  it, other accounts on the PC could read items moved there from a private profile
  (see [Threat model](32-threat-model.md)).
- Cloud items have their own quarantine inside the sync root
  ([Cloud storage](13-cloud-storage.md#removing-cloud-items)). They never enter a local
  quarantine (S3).
- A quarantine on another drive, which frees space at once, is in Next
  ([Roadmap](06-roadmap.md)).

### Batches

Every confirmed move is one batch: a folder named after its date and its purge date, for
example `2026-10-31 · purged after 2026-11-30`. It holds:

- the moved items under their own names (with a numbered suffix if two names collide)
- `batch.unhoard-manifest.json`, the manifest
- `READ ME - where these came from.txt`, in the user's language, with the original
  locations

### The manifest

The manifest is written **before** each move and flushed to disk (S5). If it cannot be
written, for example on a full disk, the move does not start.

| Field | Content |
|---|---|
| `schemaVersion` | Format version of the manifest |
| `batchId`, `createdAt`, `purgeAfter` | Batch identity and its 30-day purge date |
| `warningsShown[]` | When each companion purge warning was shown |
| `items[].id`, `operationId` | Item and operation identity |
| `items[].originalPath`, `quarantinePath` | Where it came from and where it is now |
| `items[].volumeSerial`, `fileId` | The 128-bit file ID from `FILE_ID_INFO`, for the identity check |
| `items[].kind`, `logicalBytes`, `onDiskBytes`, `fileCount` | What it is and how big |
| `items[].created`, `modified`, `attributes` | So a restore puts back the same timestamps and attributes |
| `items[].ruleId`, `tier` | Why it was offered |
| `items[].status` | `moving`, `quarantined`, `restored`, `deletedByUser` or `purged` |

The app rebuilds its list of batches from the manifests at every start, so the manifests
are the only source of truth.

### Choosing several items

Suggestions has a checkbox for each item that can move to the quarantine and for each
group, across all risk tiers. Nothing is ticked at the start (S1). A bar shows the number
and size chosen; its button opens the same preview as a single move, so the steps below
apply to all of them at once. A rescan or a move drops chosen items that are no longer
offered.

### A move, step by step

1. **Preview (dry run).** You see the items, the total size, the destination, and skipped
   items with a reason. The dry run writes nothing (T-5.9).
2. You confirm.
3. For each item, Unhoard opens it with `FILE_FLAG_OPEN_REPARSE_POINT` (plus
   `FILE_FLAG_BACKUP_SEMANTICS` for folders), so a link is never followed.
4. **Identity check:** the volume serial, the 128-bit file ID, the size and the last-write
   time must match the scan. On a mismatch the item is skipped and reported.
5. The manifest entry is written with status `moving` and flushed.
6. The rename goes through **that same handle** (`SetFileInformationByHandle`), so the item
   cannot be swapped between the check and the move.
7. The entry becomes `quarantined`, and the activity log gets one line.

Tauri commands take scan-index item IDs, never path strings from the UI. Rust resolves
each ID and re-checks the item ([Threat model](32-threat-model.md)).

### Restore

- Restore moves an item back to its original path and recreates missing parent folders.
- **It never overwrites.** If the original path exists, the item is restored next to it as
  `name (restored).ext`, and you are told.
- Names, timestamps and attributes match what they were before the move (T-5.2).

### The 30-day purge

The purge is the **only** automatic action in Unhoard, and the only place where anything
is deleted for good.

- A batch older than 30 days is purged only once its companion warning was shown at least
  3 days earlier. If the app was not opened, the purge waits.
- The companion warns 5, 3 and 1 day(s) before, in a speech bubble.
- The purge works only inside a quarantine root and only on items whose manifest entry
  still matches the file on disk (S6). An item that changed is left alone and reported.
- A link inside the quarantine that points outside it is removed as a link. Its target is
  never touched (T-5.6).
- After the purge, Unhoard re-measures free space before reporting it (S10).

### Deleting earlier yourself

The app has no delete popup of its own. "Open in Explorer" opens the quarantine folder,
and the companion explains the two ways to delete early:

- select the items and press **Shift+Delete** (permanent), or
- press **Delete** and then empty the **Recycle Bin**.

Items you delete this way are detected and shown as "deleted by you".

## Failure handling

| Case | What happens |
|---|---|
| Locked file or access denied | That item fails with its reason; the other items continue |
| File vanished or changed since the scan | Skipped and reported (identity check) |
| Long paths | All paths use the `\\?\` form internally, so paths over 260 characters work |
| Unicode names, trailing dots or spaces | Handled through the `\\?\` form, tested in move and restore (T-5.13) |
| Full disk | The manifest write fails first, so the move never starts |
| Crash or power loss mid-move | At start-up, entries left in `moving` are compared with the disk and set to `quarantined` or back to their original state |
| Junctions, symlinks, other reparse points | Never followed; a link is moved or purged as a link |

Every result is reported per item: what succeeded, what failed and why. Recovery
information is never dropped.

## The activity log

Every move, restore and purge appends one line to `%LOCALAPPDATA%\Unhoard\activity.jsonl`:
the time, the operation, the item IDs and the result. It never holds file contents. It
stays on the PC as a JSON Lines file, one record per operation.

## What Unhoard tells you never to do

The companion and the explanations warn against these common mistakes:

- deleting WinSxS, System32, the Installer folder or ProgramData internals by hand
- registry cleaners
- disabling the page file
- deleting restore points blindly

---

## Related in this cluster

- [Known locations](12-known-locations.md): where each tier and action comes from.
- [Storage map](10-storage-map.md): where you find the items to act on.
- [Cloud storage](13-cloud-storage.md): the cloud quarantine and "free up space".
- [Photo review](14-photo-review.md): photos and videos go through the same quarantine.
- [Companion](15-companion.md): the purge warnings and the Explorer guidance.

Back to the [docs hub](README.md).
