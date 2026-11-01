[Unhoard](../README.md) › [Docs](README.md) › Features › Storage map

# Storage map

The storage map answers the first question every user has: where did my space go? It is
the centre stage of the app. The default view is a **living cell map**, with a switch to a
**simple treemap**. A sorted list next to the map shows the same data and is its
accessible equivalent. Acceptance criteria: F2 and F3 in [MVP scope](05-mvp-scope.md).

## What the user sees

- **The stage:** the map of the current folder or drive. Each cell is one child item, and
  its area matches its share of the parent.
- **The breadcrumb** above the map: `C: › Users › you › Videos`. Clicking a part, or
  pressing Backspace, goes up a level.
- **The list** next to the map: the same children, sorted by size, with size, share,
  kind and risk tier. Hover and selection are synced between list and map: pointing at
  a row outlines its cell the way a selection does.
- **The details panel:** for the selected item, its size, the known-location explanation
  ([Known locations](12-known-locations.md)), its tier badge and its actions.
- **The drive bar** below the stage: used, free and "not freed yet" (items in the
  quarantine on this drive).

## Measures and units

- **Size on disk is the default.** It is what the drive actually loses: compressed files
  count less and online-only cloud files count 0. A switch in Settings shows **logical
  size** instead, which is what Explorer's "Size" column shows.
- Units are binary and labelled as Explorer labels them: 1 GB = 1024³ bytes, shown as
  "GB" ([Decisions](24-decisions.md)).
- An unknown size is shown as "unknown", never as 0.

## Colours by kind of content

Every cell is coloured by what it holds, so a glance shows what kind of content fills the
disk. Colour is never the only signal: each cell also carries a label when it is large
enough, and hidden space has a hatch pattern.

| Kind | Examples |
|---|---|
| Photos and videos | Camera rolls, phone backups, WhatsApp media, video projects |
| Apps and games | Program folders, game libraries |
| Downloads | Installers, ISOs, archives |
| System | Windows, the page file, the hibernation file, Windows.old |
| Documents | Office files, PDFs, notes |
| Developer | WSL and Docker disks, package caches, build folders |
| Cloud | Local copies of files in a sync root |
| Hidden space | What the scan could not see (hatched) |
| Other | Everything that fits none of the above |

A folder's kind is the kind that holds most of its bytes. The legend is always one click
away. The palette comes from the Holo Command look below, in a dark and a light variant,
each checked for contrast and colour-blind safety.

## Look and feel

The app uses the **Holo Command** look: a command center in the spirit of a holographic
heads-up display.

- **Dark theme:** black-navy surfaces, cyan and ice-blue light, thin glowing lines, corner
  brackets on panels and a faint scan-line texture.
- **Light theme:** the same layout and accents on pale, cool surfaces. The cells keep
  their colours with less glow. The app follows the Windows light or dark setting for the
  whole window, the stage included.
- **The stage:** translucent, glowing membranes; HUD rings around the centre; a radar
  sweep that turns fast during a scan and slowly at rest; live readouts (files, scanned,
  hidden) in the corner.
- **Type:** Segoe UI Variable for body text (part of Windows), Rajdhani for headings and
  stage labels, and JetBrains Mono for numbers and paths. Both are open-source (SIL Open
  Font License) and bundled with the app, because the app loads nothing from the network.
- The shell (navigation, lists, settings) stays calm and readable; the motion lives on
  the stage. The radar sweep turns only while a scan runs; reduced motion and the "calm"
  setting shorten every transition.

## Zoom, small items and keyboard

- Clicking a cell zooms into it. The map animates from the parent to the child layout.
- Items smaller than 0.5% of the current view are grouped into one "N smaller items"
  cell. Zooming into that cell shows them.
- Only the two visible levels are laid out at any time: the current folder's children
  and, inside each large child, a hint of its own children.
- Keyboard: arrow keys move between cells (to the nearest cell in that direction), Enter
  zooms in, Backspace goes up, the Menu key or Shift+F10 opens the actions.

## The living cell map

The cell map is a **weighted Voronoi treemap**: organic, convex cells whose areas match
the sizes. It is computed as a **power diagram**.

### Layout

For one level with n cells (at most about 50, thanks to the "smaller items" grouping):

1. Each cell *i* has a site *pᵢ* and a weight *wᵢ*. Its region is the parent polygon
   clipped by one half-plane per other cell *j*:
   `2x·(pⱼ − pᵢ) ≤ |pⱼ|² − |pᵢ|² − wⱼ + wᵢ`.
   All cells are convex, so Sutherland–Hodgman clipping is enough.
2. Each iteration moves every site to its cell's centroid and adapts its weight with a
   damped Newton step: `w ← w + ½ · (target − area) · 4r / perimeter`, where
   `r = √(area / π)`. Raising a weight by *d* moves each edge outwards by about
   *d* / (2 · neighbour distance), so this step closes about half the area gap per
   iteration. A cell that vanished gets the largest weight back and recovers.
3. If one weight overpowers a neighbour (`wᵢ − wⱼ > dist²`), it is clamped, so no cell
   vanishes. In the design explorations this converges to within 2.5% of every target
   area.
4. Levels nest: the children of a cell are laid out inside that cell's polygon.

Starting sites come from a seeded pseudo-random generator keyed on the item ID, so the
same input always gives the same layout (T-2.4).

### How it moves

The layout runs **one relaxation step per frame**, starting from the previous frame's
state:

- **During a scan** the target sizes change as results arrive, and the cells grow towards
  them smoothly.
- **At rest** the relaxation runs until the layout has settled and then stops, cells
  inside large cells included. The map stands still and redraws only on interaction, so
  nothing moves while the user reads it. The area stays within 3% of the true share
  (AC2.2).
- **Reduced motion** (Windows animation effects off, or the "calm" motion setting):
  transitions last at most 200 ms.

### Rendering

- Canvas 2D first. Each cell is drawn as a membrane: the polygon inset by a small gap,
  with rounded corners, a radial gradient and an optional soft nucleus glow.
- If the frame-rate target fails (AC2.9: 60 fps with 300 visible cells on integrated
  graphics), effects are reduced first: glow, then gradients. Moving to WebGL (PixiJS) is
  considered only if that is still not enough.
- Labels are drawn only on cells large enough to hold them.

## The simple treemap

- A **squarified treemap** (d3-hierarchy), with the same colours and labels.
- The switch is remembered (AC2.1).
- **Morph:** switching views resamples each old and new polygon to the same number of
  points and tweens between them for about 600 ms, so the eye can follow each item.

## Priority matrix

The third view answers "where do I start?". Every suggestion is a bubble:

| Channel | Meaning | Scale |
|---|---|---|
| x | Size on disk: the space it frees | Log, from 1 MiB to the largest item, so one huge item does not squash the rest |
| y | Effort, in four levels | 1 = one click in Unhoard; 2 = review first; 3 = through another app or a Windows tool; 4 = needs admin rights |
| Bubble size | Days since last use | Log, from a day to ten years |
| Colour | Kind of content | The map colours |

Bottom right holds the **quick wins** (big and easy); top right the **big jobs**. Items
Unhoard only explains (Keep tier, protected) are left out.

"Last use" comes from the best source for each kind, and the tooltip names it:

- Steam games: "last played" from the game's Steam manifest.
- Programs: the last start from Windows' program history (the UserAssist registry key,
  readable without admin).
- Everything else: the newest change of any file inside.

## Hidden space

For a whole-drive scan, **hidden space = (total − free) − scanned size on disk**. It is
never negative (T-3.1). It shows as one hatched cell with its size.

- **What usually hides there:** restore points and shadow copies in System Volume
  Information, other users' profiles, folders the current user may not read, and the
  NTFS metadata files.
- Parts that can be measured without admin are named and subtracted, for example the
  current user's Recycle Bin.
- **"Reveal hidden space"** starts the elevated read-only helper after a UAC prompt
  ([Architecture](20-architecture.md#the-elevated-helper)). It scans only the unscanned
  folders plus System Volume Information. Its results replace part of the hatched cell
  with named cells; what remains stays hatched.
- If the user declines UAC, nothing changes and the companion explains what that means.
- The revealed folders are grouped by kind (restore points, Microsoft Store apps,
  recovery, Windows Defender, error reports, other accounts, other), each with what it is
  and what to do; folders under 1 MiB are summed into one line.

### Reading protected folders

Some folders cannot be read without admin rights; their size is unknown. The setting
**Read protected folders** decides what happens:

| Setting | What happens |
|---|---|
| Ask (default) | When a scan starts, the companion asks once and says why. With Yes, the helper starts (one UAC prompt) and reads every unreadable folder when the scan ends |
| Always | The helper starts with every scan, without the question (Windows still shows UAC) |
| Never | No question |

Whatever the setting, an unreadable folder's details offer **Read with admin rights**
for that folder. What the helper reads fills the map; such items are marked "read with
admin rights" and never offer a move, because the app itself has no rights there.

## Cloud content

- Online-only files take 0 bytes on disk, so in the default view they take no area.
- A **cloud view** switch shows cloud cells as ghost outlines at their logical size, so the
  user can see the size of the cloud library without it hiding the local picture.
- Details on placeholder states: [Cloud storage](13-cloud-storage.md).

## Live build-up during a scan

- The first results reach the map within 1 second. The scan reports progress at most
  every 250 ms (AC1.3); the map, the list and the details reload every 1.5 seconds, so the
  cells grow calmly instead of jumping at every batch.
- A progress bar runs along the top of the stage. For a drive scan it shows the scanned
  size against the drive's used space, as a percentage; it stays below 100% until the scan
  ends, because hidden space is never scanned. A folder scan has no known total, so its
  bar moves without a percentage and shows the size found so far.
- The UI never receives the whole tree. It asks the core for a view: the node ID, the
  depth and the top N children plus "others" ([Architecture](20-architecture.md#ipc)).
- A partial scan is labelled "partial" in the breadcrumb and the drive bar.

---

## Related in this cluster

- [Cleanup and safety](11-cleanup-safety.md): what happens when you act on a cell.
- [Known locations](12-known-locations.md): the explanations in the details panel.
- [Cloud storage](13-cloud-storage.md): placeholders and the cloud view.
- [Photo review](14-photo-review.md): reviewing the photos and videos a cell holds.
- [Companion](15-companion.md): the guide that explains what you see.
- [Optimise](16-optimise.md): the check-up that links back to the map.

Back to the [docs hub](README.md).
