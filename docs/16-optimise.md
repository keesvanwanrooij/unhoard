[Unhoard](../README.md) › [Docs](README.md) › Features › Optimise

# Optimise

Optimise (Dutch: "Optimaliseren") is an **honest check-up**: a diagnosis of where
performance, space and system health are lost, with a few safe and reversible actions.
It is never a button that promises a faster PC. Optimise is part of the insights phase
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
| CPU | One rescan of the system drive per day. Measured: 124 CPU-seconds for a 1 TB drive with 2.8 million files, about 0.14% of one core averaged over a day | 0.29% of one core (250 s) | About 1% of one core at idle priority | **250 CPU-seconds per measurement (0.29% of one core per day)** |
| Local storage | Measured: 55 KB per snapshot of 1,000 folders. Daily snapshots for 90 days, then one per week, kept for 2 years: about 194 snapshots, 10.7 MB | 21 MB | Hundreds of MB | **20 MB, oldest snapshots dropped first** |
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

## Data sources

Each check reads a documented Windows source. Checked on a Windows 11 PC without admin
rights on 2026-11-13 where marked.

| Check | Source | Without admin |
|---|---|---|
| Top consumers of CPU and memory | Process list (`CreateToolhelp32Snapshot`), two samples a second apart for CPU (`GetProcessTimes`), working set (`K32GetProcessMemoryInfo`) | Own processes in full; other users' and system processes are left out |
| Memory pressure and page file | `GlobalMemoryStatusEx`, page-file usage from `GetPerformanceInfo` | Yes |
| Disk health status | Storage Management API, `MSFT_PhysicalDisk.HealthStatus` | Yes (checked) |
| Wear, temperature, error counts | `MSFT_StorageReliabilityCounter` | **No** (checked): Unhoard opens Settings › Disks, where Windows shows them |
| Startup apps | The `Run` keys of the user and the machine, the Startup folders, and their `StartupApproved` entries | Yes (checked: 13 entries) |
| Pending updates | Windows Update Agent (`IUpdateSearcher`, `IsInstalled=0 and IsHidden=0`) | Yes (checked) |
| Update errors | The Windows Update history from the same API | Yes |
| System-file integrity | Not readable without admin; Unhoard explains the two commands to run | – |
| Page file | The `PagingFiles` value under `Memory Management` in the registry | Yes |
| Power mode | The active overlay scheme under `Control\Power\User\PowerSchemes` in the registry | Yes |

Task Manager's "startup impact" rating has no public API, so Unhoard does not show it.
Each startup app shows its command and whether its program file still exists. TRIM status,
the last "Optimize drives" run, process disk activity and update errors per update are not
shown in this phase: they need admin rights or a performance-counter query that adds
little.
Startup impact over time comes from the background insights only when Windows offers it
through a public API; until then that line is left out.

## Acceptance criteria

- AC16.1 The Optimise screen (EN "Optimise", NL "Optimaliseren") lists findings in the six
  groups, each with its state, measurement, source, reason and next step, and no score.
- AC16.2 Every check only reads, except the startup toggle.
- AC16.3 The startup toggle writes only the `StartupApproved` value Task Manager writes,
  shows a preview first, can be undone with one click, and writes an activity-log entry.
  Machine-wide entries that need admin are explained and point to Task Manager.
- AC16.4 A startup entry whose file no longer exists is flagged and never deleted.
- AC16.5 A disk finding shows the health Windows reports and opens Settings › Disks for
  wear, temperature and warnings, which need admin rights to read.
- AC16.6 The check-up shows its findings after about a second (one second of CPU
  sampling). The Windows Update search, which can take half a minute, runs on its own and
  shows its finding when it is done; the screen stays usable meanwhile.
- AC16.7 The companion explains the myths (freeing RAM, registry cleaners, disabling the
  page file) when the user opens a related finding.

| ID | Test | Kind |
|---|---|---|
| T-16.1 | each finding's state follows from its measurement for generated inputs (thresholds per check) | core unit |
| T-16.2 | the startup toggle writes the disabled and enabled byte patterns Windows uses, and undo restores the exact old value | platform unit (temporary registry key) |
| T-16.3 | a startup entry pointing to a missing file is flagged and left in place | core unit |
| T-16.4 | the dry run of the startup toggle writes nothing | platform unit |

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
