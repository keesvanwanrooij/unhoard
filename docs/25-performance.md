[Unhoard](../README.md) › [Docs](README.md) › Engineering › Performance

# Performance

The minimum speed at which Unhoard feels pleasant to use, and the tests that catch a
change that makes it slower. Every budget below is a hard limit: a change that breaks one
is not done. Acceptance criteria that refer to these budgets carry the ID P1–P14.

## Why these numbers

The budgets follow well-established limits of human perception:

- **About 100 ms** feels instant. Anything a click or key press causes must start to show
  within that time.
- **About 16.7 ms per frame** (60 frames per second) feels smooth. A frame that takes
  longer is a visible stutter.
- **Work over 50 ms on the UI thread** blocks input and is felt as lag, so long work is cut
  into smaller pieces or moved off the UI thread.
- **About 1 second** keeps the train of thought. Longer work shows progress.
- **About 10 seconds** is where attention breaks. Longer work can be cancelled.

## Reference PC

Budgets are measured on a **mid-range PC**: a 4-core CPU from the last five years,
integrated graphics, 8 GB of memory, an NVMe SSD, a 1080p screen at 100% scaling, and
Windows 11. In automated UI tests this is emulated by slowing the CPU down 4 times
(Chromium CPU throttling), because CI runners are faster but noisier.

The test data is a **generated tree of 1 million entries**, with a realistic depth and
size spread, never a real disk.

## Budgets

All times are the 95th percentile (p95) over at least 20 repetitions, measured from the
input event to the moment the result is painted.

### Interaction

| ID | Interaction | Budget |
|---|---|---|
| P1 | Visible response to any click or key press (pressed state, selection, focus) | ≤ 50 ms |
| P2 | **Switching screens** (Overview, Suggestions, Quarantine, Cloud, Photos, Settings): the new screen's content is painted | ≤ 100 ms |
| P3 | Switching screens: the transition animation has ended | ≤ 250 ms. Input during the animation is accepted at once; nothing waits for an animation to end |
| P4 | Hover in the map or the list: highlight and tooltip | ≤ 50 ms |
| P5 | **Zoom into a cell** or go up a level: the animation starts | ≤ 100 ms |
| P6 | Zoom: the new level is settled | ≤ 600 ms |
| P7 | Map and treemap switch: the morph starts | ≤ 100 ms (the morph itself lasts about 600 ms) |
| P8 | Search: results for a typed query over 1 million entries | ≤ 150 ms after the last key press |
| P9 | Opening the move preview (dry run) for up to 1,000 items | ≤ 500 ms; above that a progress indicator shows |

### Smoothness

| ID | Measure | Budget |
|---|---|---|
| P10 | Frame time of the map at rest and during zoom, with up to 300 visible cells | p95 ≤ 16.7 ms (60 fps); no frame over 50 ms |
| P11 | Long tasks on the UI thread during any interaction | none over 50 ms |
| P12 | Scrolling a list of 10,000 rows (virtualised) | p95 frame ≤ 16.7 ms |

### Core and IPC

| ID | Operation | Budget |
|---|---|---|
| P13 | A view query from the UI to the core and back (node, depth, top N plus "others") on the 1-million-entry tree | ≤ 30 ms, IPC included |
| P14 | Re-aggregating totals after a subtree rescan of 10,000 entries | ≤ 50 ms |

The start-up, scan-speed, memory and frame-rate targets in
[MVP scope](05-mvp-scope.md#non-functional-requirements) apply as well: the window shows
within 2 seconds, the first scan results appear within 1 second, progress arrives at most
every 250 ms, and a full scan of about 1 million files finishes in under 60 seconds.

### Long work

- Anything that may take over **1 second** shows progress.
- Anything that may take over **10 seconds** can be cancelled, and cancel takes effect
  within 1 second.

## How the budgets are kept by design

- The UI never receives the whole tree; it asks for views ([Architecture](20-architecture.md#ipc)).
- Screens keep their last view in memory, so switching back paints at once while fresh
  data loads.
- Lists are virtualised.
- The cell-map layout runs one relaxation step per frame, never a full solve on the UI
  thread.
- Transitions are interruptible and never block input.
- Heavy work (scanning, hashing, aggregation) runs in Rust worker threads.

## How the budgets are tested

### Core benchmarks (Rust)

- Benchmarks with **criterion** in `crates/core/benches/`: view query, aggregation,
  rescan of a subtree, rule matching, search, and the identity-check path on the fake
  platform layer.
- Each benchmark builds the generated 1-million-entry tree first.
- Run with `cargo bench -p unhoard-core`.

### UI benchmarks (TypeScript)

- **Vitest bench** for pure functions: one cell-map relaxation step, the treemap layout,
  the polygon morph and the "smaller items" grouping, each with 50 and 300 cells.

### Interaction tests (the real UI)

- **Playwright** drives the built UI in Chromium, with the IPC layer replaced by a fake
  core that serves the generated tree. The CPU is throttled 4 times.
- Scripted scenarios: switch every screen 20 times, zoom in and out 20 times, toggle map
  and treemap, hover across the map, scroll a long list, type a search.
- The UI marks each step with the User Timing API (`performance.mark` and
  `performance.measure`), and a `PerformanceObserver` records long tasks and frame times.
  The marks exist only in the performance build.
- The scenarios write `perf-report.json` with the p95 of every budget ID.
- Run with `npm run perf`.

### Regression check

- `perf/baseline.json` holds the p95 numbers of `main`, committed to the repository.
- `npm run perf:check` compares the latest reports (Rust and UI) with the budgets and the
  baseline:
  - **over a budget:** fail
  - **more than 20% slower than the baseline:** fail in CI (runners are noisy)
  - **more than 10% slower than the baseline:** a warning locally, so you notice it while
    you work
- The baseline is updated only on purpose, in a pull request that says why the new number
  is acceptable.

### When to run it

- **While you work** on a feature that touches the UI, the core's hot paths or IPC: run
  `npm run perf` before you start, to get your own numbers, and again after each step. A
  warning means stop and look before going further.
- **In CI:** the `perf` job runs on every pull request that touches `src/`, `crates/core/`
  or `src-tauri/`.
- **Before a release:** the manual check measures P2, P5 and P10 once in the real app on a
  mid-range PC ([Development](22-development.md#manual-checks)).

---

## Related in this cluster

- [Architecture](20-architecture.md): the IPC views and the worker threads behind these numbers.
- [Development](22-development.md): the checks and the manual checks.
- [Tech stack](21-tech-stack.md): criterion, Vitest bench and Playwright.
- [Decisions](24-decisions.md): why the budgets are what they are.

Back to the [docs hub](README.md).
