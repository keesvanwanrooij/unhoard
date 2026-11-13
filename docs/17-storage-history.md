[Unhoard](../README.md) › [Docs](README.md) › Features › Storage history

# Storage history

Storage history answers "what grew, and why?". Unhoard keeps a small record of how large
the biggest folders were over time, shows the growth as a timeline, and names what grew:
"WSL's Remote Desktop traces grew by 54 GB since Tuesday". It is the answer to the first
question of the [Vision](01-vision.md) told over time instead of at one moment.

## Where the measurements come from

| Source | When | What it records |
|---|---|---|
| **Every scan you run** | At the end of each drive or folder scan | A snapshot of the scanned root |
| **The daily measurement** | Once a day, in the background, on by default | A snapshot of the system drive |

A **snapshot** holds, for the largest folders, their size on disk and file count:

- up to 1,000 folders, at most three levels below the drive root, plus every folder a
  known-location rule or definition matched within those levels and one level deeper
  (rule matches go first when the limit is reached)
- the drive's used and free space, and the hidden space
- no file names below those folders and no file contents

The daily measurement follows the [Optimise](16-optimise.md#background-insights) limits:
at most 250 CPU-seconds per run (0.29% of one core averaged over a day), two scan threads,
background CPU, I/O and memory priority, and no run on battery saver or while a
full-screen app runs. A run that reaches its budget stops and writes no snapshot; the next
day's run tries again.

### Running without the app open

The daily measurement runs whether or not Unhoard is open:

- When background insights are on, Unhoard registers a **scheduled task for the current
  user** ("Unhoard daily measurement") in Windows Task Scheduler. It needs no admin. It
  starts `unhoard-app.exe --measure` once a day when the PC is idle, which measures,
  writes the snapshot and exits; it shows no window.
- Turning background insights off in Settings removes the task. The uninstaller removes
  it too.
- When the app starts and the last snapshot is more than a day old, it measures right
  away, so a PC that is rarely idle still gets its points.

## Storage

Snapshots live in `%LOCALAPPDATA%\Unhoard\insights\history.jsonl`, one JSON line per
snapshot, appended. Paths in the file are stored as they are, because the file never
leaves the PC; an export replaces the profile path (S9).

- **Retention:** every snapshot for 90 days, then one per week (the last of each week)
  for two years.
- **Size cap:** 20 MB. When it is reached, the oldest snapshots go first.
- **Compact lines:** each folder is stored as its name and the index of its parent, so a
  snapshot of 1,000 folders takes about 55 KB.
- **Compaction** rewrites the file at most once a day, to a temporary file that replaces
  the old one only when it is complete, so a crash never loses the history.
- A line that cannot be read is skipped and counted, never fatal.

## Growth

Growth compares two snapshots of the same root: the **newest** and a chosen **baseline**
(a day, a week, a month or three months back; the nearest snapshot at or before that
moment).

- A folder's growth is its size now minus its size then. A folder present in only one of
  the two counts from zero.
- Growth is attributed to the **deepest folder that explains it**: when
  `AppData\Local\Temp` grew by 54 GB and almost all of that is in
  `Temp\DiagOutputDir\RdClientAutoTrace`, the line names the deepest folder that holds at
  least 80% of the parent's growth.
- Each growing folder carries its explanation from the [known-locations
  library](12-known-locations.md) (rule or definition) or the [generated
  explanation](12-known-locations.md#explanations-for-everything-else), so the timeline
  says what it is, not just where it is.
- Shrinking is shown too ("the quarantine was emptied: −31 GB").
- Growth under 100 MB is not listed; it is noise on a drive of hundreds of gigabytes.

## The History screen

A screen of its own in the navigation (EN "History", NL "Geschiedenis"):

- **Timeline:** used space of the drive over time, with a point per snapshot; scans you
  ran and daily measurements look different. Hovering a point shows the date and the
  used, free and hidden space.
- **Period switch:** day, week, month, three months.
- **What grew:** the largest growers in the chosen period, each with its size then and
  now, the growth, a sparkline of its own history and its explanation. A click selects
  the item in the overview (if the current scan has it).
- **On the map:** a "Growth" switch on the overview colours each cell by its growth in
  the chosen period instead of by kind of content: warm for growth, cool for shrinking,
  grey for no data.
- With fewer than two snapshots the screen says so and explains how history fills up.

## The companion

- After a scan, when something grew by at least 5 GB or 50% since the previous snapshot,
  Lumen names the largest grower and offers to open History.
- When the daily measurement sees fast growth (at least 10 GB in a day in one folder), the
  next time the app opens Lumen says so. There is no tray icon and no notification
  outside the app.

## Acceptance criteria

- AC17.1 Every finished scan writes a snapshot of its root; a cancelled scan writes none.
- AC17.2 With background insights on, a scheduled task for the current user runs the daily
  measurement without admin; turning them off removes the task and stops new snapshots.
- AC17.3 Snapshots hold only the folders, sizes and counts listed above, never file names
  below those folders or file contents.
- AC17.4 Retention and the 20 MB cap hold; compaction never leaves a half-written file.
- AC17.5 Growth between two snapshots is attributed to the deepest folder holding at least
  80% of it, with that folder's explanation.
- AC17.6 The History screen shows the timeline, the period switch and the growers; the
  map's Growth switch colours cells by growth.
- AC17.7 The daily measurement stays within the Optimise limits and never runs on battery
  saver.

## Tests that come first

| ID | Test | Kind |
|---|---|---|
| T-17.1 | a snapshot of a generated tree holds the top folders, rule matches and drive totals, and no names below them | core unit |
| T-17.2 | retention keeps daily points for 90 days and weekly ones for two years; the cap drops the oldest first | core unit |
| T-17.3 | compaction is atomic: a failure midway leaves the old file intact | core unit (fake) |
| T-17.4 | growth is attributed to the deepest folder holding 80% of it; shrinking is negative; new folders count from zero | core unit |
| T-17.5 | a broken line in the history file is skipped and counted | core unit |
| T-17.6 | the period switch picks the nearest snapshot at or before the baseline | UI unit |

---

## Related in this cluster

- [Optimise](16-optimise.md): the background insights and their limits.
- [Storage map](10-storage-map.md): where the Growth colouring shows.
- [Known locations](12-known-locations.md): the explanations growth lines carry.
- [Companion](15-companion.md): what Lumen says about growth.

Back to the [docs hub](README.md).
