[Unhoard](../README.md) › [Docs](README.md) › Features › Optimise

# Optimise

Optimise (Dutch: "Optimaliseren") is an **honest check-up**: a diagnosis of where
performance, space and system health are lost, with a few safe and reversible actions.
It is never a button that promises a faster PC. Optimise is in **Next**, not in the MVP
([Roadmap](06-roadmap.md)).

## How it works

1. **Diagnose.** Every check only reads data.
2. **Recommend.** Each finding says what was measured, where the number comes from, why
   it matters and the safe next step.
3. **Act, optionally.** A few findings offer an action. Each action has a dry-run
   preview, is reversible and writes an activity-log entry.

### Output

- A list grouped by area. **No score**, because a single number hides what matters.
- Each finding has one of three states: **in order**, **worth a look** or **action
  possible**.
- Each finding shows: the measurement, its source ("as measured by Windows"), why it
  matters, and the next step.
- Each action names its risk class from the engineering standards
  ([AGENTS.md](../AGENTS.md)). The startup toggle is High.

## The six groups

### 1. System performance

- A snapshot of the top 5 consumers of CPU, memory and disk.
- Memory pressure and page-file use.
- An explanation, plus a button that opens Task Manager. Unhoard never ends a process
  itself.

### 2. Storage performance

- Disk health as Windows reports it: wear, temperature and warnings, as in Settings ›
  Disks. The source is the Storage Management API (`MSFT_PhysicalDisk` and
  `MSFT_StorageReliabilityCounter`) (verify which counters work without admin).
- Response time and active time, as a snapshot.
- TRIM and "Optimize drives" status, and fragmentation on hard disks, with a link to the
  Windows tool.

### 3. Startup and background

- Startup apps, with Windows' own startup-impact rating.
- **Action:** disable or enable a startup app the way Task Manager does (the
  `StartupApproved` values), reversible with one click, with an activity-log entry.
- Startup entries that point to files that no longer exist are flagged, never deleted.
- Apps whose caches or data grow fast, from the background insights.

### 4. Storage use

- Enough free space for Windows Update.
- Temp files, caches, crash dumps, update leftovers and duplicates.
- These link into the existing features: [known locations](12-known-locations.md), the
  [quarantine](11-cleanup-safety.md#the-quarantine) and [photo review](14-photo-review.md).
  There is no second cleaner.

### 5. Windows health

- Pending updates, update errors, and system-file integrity status where it can be read
  without admin (verify).
- Unhoard explains, opens the right Windows tool, or copies the command for you to run
  (`sfc /scannow`, `DISM /Online /Cleanup-Image /RestoreHealth`). It runs no repair
  itself, so the main app never needs elevation (S7).

### 6. System configuration

- Power mode, the page file (for example "turned off"), and driver or firmware notices
  such as "optional driver update available".
- Unhoard signals and links to the Windows setting. It changes nothing.

## What Optimise never does

- registry cleaning (the only registry write is the startup toggle that Windows itself
  uses)
- "freeing" RAM, disabling services, tweaks, installing drivers
- promising a percentage of speed

The companion explains these myths instead. For example: freeing RAM does not help,
because Windows uses free memory as a cache and gives it back the moment an app needs it.

## Background insights

Some insights need measurements over time: storage growth per top folder, startup impact
over time and disk health readings. They come from light background measurements.

- **On by default**, because they carry real value. The first run says plainly that they
  are on and how to turn them off. A switch in Settings turns them off at any time.
- **Local only.** No network (S8 holds), nothing leaves the PC.
- **What is stored:** per day, the on-disk size and file count of the largest folders
  (up to 1,000, at most three levels deep), the startup-impact list and the disk health
  counters. No file names below those folders, and no file contents.

### Limits

A background feature that makes the PC slower is a feature people uninstall. Each limit
is the **smaller of two numbers**: twice what the work is expected to need, and what an
average PC can give without anyone noticing.

| Resource | Expected need | Twice that | What an average PC can spare | Limit |
|---|---|---|---|---|
| CPU | One rescan of the system drive per day, about 2 CPU-minutes, plus seconds for startup and disk readings: about 0.15% of one core averaged over a day | 0.3% of one core | About 1% of one core at idle priority | **0.3% of one core, averaged over a day** |
| Local storage | About 16 KB per daily snapshot. Daily snapshots for 90 days, then one per week, kept for 2 years: about 3 MB | 6 MB | Hundreds of MB | **6 MB, oldest snapshots dropped first** |
| Retention | Year-on-year growth needs at least 13 months | – | – | **2 years** |

How the limits are kept:

- One worker thread at idle CPU priority and background I/O priority.
- No runs on battery saver, and a pause while a full-screen app or game runs.
- If the daily rescan reaches its CPU budget before it ends, it stops and continues the
  next day.
- The expected numbers are estimates. The build measures them on a generated tree and
  sets the limits again with the same rule ([Decisions](24-decisions.md)).

The background insights tie in with the Next items "Background insights with a tray icon"
and "Storage history". What they store is listed in [Privacy](30-privacy.md), and the
threats are in the [Threat model](32-threat-model.md).

## Messaging

Measured facts, never claims:

- Yes: "3 of your 14 startup apps have high impact, as measured by Windows."
- No: "Speed up your PC by 300%."

---

## Related in this cluster

- [Known locations](12-known-locations.md): what the storage-use checks link to.
- [Cleanup and safety](11-cleanup-safety.md): the quarantine and the activity log.
- [Companion](15-companion.md): the myth explanations and background notifications.
- [Storage map](10-storage-map.md): where storage growth will show.

Back to the [docs hub](README.md).
