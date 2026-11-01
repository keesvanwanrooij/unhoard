[Unhoard](../README.md) › [Docs](README.md) › Engineering › Decisions

# Decisions

Every current decision with its reason. When a decision changes, its row is rewritten,
not stacked: git holds the history. Hard-to-reverse choices are proposed here first, with
options and trade-offs, before they are built.

## Project

| Decision | Reason |
|---|---|
| The name is **Unhoard**, the repository `keesvanwanrooij/unhoard` | Says what the app helps you do; short and available |
| License **GPL-3.0-or-later**; `LICENSE` holds the verbatim GPL-3.0 text | Keeps every fork open. Verbatim text lets GitHub detect the license and stays legally exact |
| Copyright line "Copyright (C) 2026 Kees van Wanrooij" | The maintainer holds the copyright of the original work |
| Contributions use the **DCO** sign-off, no CLA | Light for contributors and enough for a GPL project |
| AI-assisted contributions are allowed, with disclosure and tests | Honest about how code is written; the tests carry the trust |
| Everything is free; funding through donations (GitHub Sponsors, set up later) | The app is for home users; value and visitors come first |
| A possible later subscription only covers cloud-AI credits | API calls cost money; the app itself and the local model stay free |
| `AGENTS.md` is the public onboarding for humans and AI agents | One place for the working rules of anyone who contributes |
| Conventional Commits, and the commit body names the risk class and F or S IDs | Readable history and traceability from requirement to test |
| On creating the GitHub repository: branch protection, secret scanning with push protection, Dependabot alerts, CodeQL, private vulnerability reporting and Discussions | Safety on the public repository from day one, and a place for questions outside issues. Applied by the maintainer when the repository is created |

## Product

| Decision | Reason |
|---|---|
| Primary audience: home users with a full disk and a large photo library; second: power users and developers | The first question "where did my space go?" is theirs, and no current tool answers it with explanations |
| Windows 10 and 11 only, with a portable architecture | Where the audience is; the platform layer keeps macOS and Linux possible later |
| Works without admin; elevation only for documented opt-in features | Most home users should never see a UAC prompt for a scan |
| The **elevated read-only "Reveal hidden space"** rescan is in the MVP | Hidden space is often the biggest unexplained block; a read-only, short-lived helper keeps the risk low |
| Unhoard **never selects anything by itself**; every move is a user click (S1) | Trust: the user stays in control |
| The **organic cell map is the default view**, with a simple treemap switch | The living map makes "where did it go" fun and memorable; the treemap stays for people who want it plain |
| Motion lives in the scan and the transitions: cells grow while the scan runs, then the map **stands still**; the radar sweeps only during a scan; "calm" and reduced motion shorten transitions | A map that keeps moving is hard to read and was experienced as restless |
| During a scan the map, list and details reload every 1.5 seconds, and cells move one layout step per frame | Faster updates made the cells jump, and three seconds felt slow; 1.5 seconds shows the drive filling in while the cells still glide |
| A drive scan's progress is the scanned size against the drive's used space, held below 100% until the scan ends | The used space is the only total known in advance; hidden space is never scanned, so the bar would otherwise stall short of the end |
| The companion shows one bubble and never a stack: newer messages replace older ones, progress updates in place, and closing a bubble silences its topic until the operation ends | Stacked, repeated progress bubbles buried the message the user wanted and came back right after being closed |
| The companion can be dragged anywhere on the stage, and its spot is remembered on this PC | The best spot depends on what the user is looking at; a fixed corner can cover a cell |
| A third view, the **priority matrix**: x = size on disk, y = effort in four levels, bubble size = days since last use, colour = kind | Answers "where do I start": quick wins sit bottom right, big jobs top right |
| The matrix uses **log scales** for size and days, and four fixed effort levels | Sizes and ages span many orders of magnitude; a log scale keeps one huge item from squashing the rest and stays honest about ratios, where percentiles or z-scores would hide or distort them |
| "Last use" comes from the best source per kind: Steam's last played, Windows' program history (UserAssist), else the newest change inside | Each is the most accurate date available without admin; the source is shown |
| Colour by kind of content | A glance shows what fills the disk, not just how big things are |
| A calm, readable shell around a show-piece centre stage | Balances a futuristic command-center look with ease of use |
| The look is **Holo Command**: black-navy, cyan and ice hologram, HUD rings and a radar sweep on the stage ([Storage map](10-storage-map.md#look-and-feel)) | Chosen from four design explorations; it is the clearest command-center feel |
| The app **follows the Windows light or dark setting** for the whole window, stage included | Respects the user's system choice; Holo Command gets a light variant |
| Type: Segoe UI Variable for body text, bundled Rajdhani for headings and stage labels, bundled JetBrains Mono for numbers and paths | Native readability for long text, character on the stage; bundled because the app loads nothing from the network; both fonts are under the SIL Open Font License |
| A **companion character** with speech bubbles, never popups; scripted in the MVP | Guidance that does not interrupt; AI later |
| The companion is **Lumen**, an original holographic orb ([Companion](15-companion.md)) | Fits the Holo Command look; an original character, not a copy of any existing assistant |
| The first-run scan is the **system drive** | That is where the problem nearly always is; other drives are one click away |
| English and Dutch; the language follows Windows with an English fallback, switchable | The maintainer's audience starts in the Netherlands; English reaches everyone else |
| **Optimise** is an honest check-up in Next: no score, measured findings, few reversible actions | "PC optimisers" are a category full of false promises; honesty is the advantage |
| Optimise **background insights are on by default**, switchable, local only | Growth over time is valuable only if it is measured from the start; the first run says it is on and how to turn it off |
| Background-insight **limits are the smaller of twice the expected need and what an average PC can spare**: 0.3% of one core per day, 6 MB, 2 years | A background feature that slows the PC gets uninstalled; 2 years allows year-on-year comparison. The build measures the real numbers and applies the same rule |
| A small CLI in the MVP (`unhoard scan <path> --json`) | Power users and scripts; it also tests the core without the UI |

## Cleanup and safety

| Decision | Reason |
|---|---|
| Removal goes through Unhoard's **own central quarantine** with manifest, restore and a 30-day purge | Centralises clutter from all over the PC and makes every move reversible |
| **One quarantine folder per drive**, `X:\Unhoard Quarantine\`, shown as one central view | A move is always a rename on the same volume: instant, no copy, no extra space |
| The quarantine folder name is `Unhoard Quarantine` in every language | Paths and manifests stay valid when the user switches language |
| The quarantine folder gets an explicit, non-inherited ACL for the current user, SYSTEM and Administrators | Without it, other accounts on the PC could read items moved there from a private profile |
| Retention is **30 days**, then the app purges | Long enough to notice a mistake, short enough to free the space |
| A batch is purged only once its warning was shown at least 3 days earlier; warnings show at 5, 3 and 1 day(s) | Nobody loses files because they did not open the app |
| **No delete-now button**; deletion happens only in the purge; early deletion is done by the user in Explorer, with companion guidance | An extra delete dialog adds risk without adding value; Explorer is familiar |
| No "clean now" for Safe-tier caches either; for space right away, Unhoard links to Storage Sense and Disk Cleanup | One path for every removal keeps the safety rules simple |
| A dry-run preview before every move, purge and bulk "free up space" | The user sees exactly what will happen; tests prove the dry run writes nothing |
| An identity check (volume serial, 128-bit file ID, size, last write) right before acting, then a rename through the same handle | Prevents acting on a file that was swapped or changed after the scan |
| Destructive Tauri commands take scan-index item IDs, never paths | The webview is untrusted |
| **Restore never overwrites**; on a conflict it restores as `name (restored).ext` | Restoring must never destroy a newer file |
| A local **activity log** of every move, restore and purge | The user can always see what Unhoard did and when |
| Risk tiers Safe, Likely safe, Review, Keep (system), always with a reason | Simple enough for home users, honest enough for power users |
| Quarantine on another drive is in Next | Same-drive renames are the safe base; cross-volume copy and verify needs its own tests |
| Reading protected folders: a setting (Ask, Always, Never), one question when a scan starts, and a per-folder button | Complete numbers when the user wants them, without a UAC prompt they did not expect; the main app still never runs elevated |
| Items read with admin rights never offer a move | The app itself has no rights there; a move would fail or need elevation, which only the read-only helper has |

## Known locations

| Decision | Reason |
|---|---|
| The library is data: TOML rules in `data/known-locations/`, text in the locale files | Contributors add entries without Rust; every rule gets a fixture test |
| `12-known-locations.md` is its own doc | It is a large, community-curated list |
| The most specific rule wins; on a tie the strictest tier | A Keep item is never shown as safer than it is |
| Unmatched large items get a generic explanation and tier Review | Every big item still gets context, without guessing it is safe |
| App knowledge has three layers: Windows' list of installed programs (automatic), a large curated library of app data and cache folders, and adapters for apps with their own records (Steam, Ollama, Docker) | The automatic layer covers nearly every program without upkeep; the curated library adds what an app's data and caches are; adapters add what only the app knows |
| Adapters may run an installed tool's read-only command after a click (`docker system df`), from the tool's install folder, with fixed arguments | Exact numbers about a container disk are only available from the tool; a fixed command from a known folder keeps it safe |
| Suggestions with the same rule are grouped into one expandable row | Dozens of browser-profile caches or build folders hid the items that matter |
| Project rules (`node_modules`, `.venv`, `target`, `bin`, `obj`) never match inside app folders (`%LOCALAPPDATA%`, `%APPDATA%`, Program Files, `%PROGRAMDATA%`), hidden folders in the profile or another `node_modules` | There they are what an installed app or extension runs from; moving them breaks the app |
| Suggestions count size on disk and leave out items under 1 MiB | An online-only file or an empty cache frees nothing; hundreds of tiny entries hid the ones that matter |

## Cloud

| Decision | Reason |
|---|---|
| The MVP reads the local sync folder; no sign-in | No network and no account needed for the main value |
| Only OneDrive personal is tested; other Cloud Files API providers are labelled "detected, not tested" | Honest about what was verified |
| **Never hydrate** an online-only file (S2) | A photo library can be larger than the free space |
| Cloud files never enter the local quarantine (S3) | Moving them out of the sync root would download them and delete them from the cloud |
| Cloud items go to a **cloud quarantine** inside their sync root; the purge deletes them there, into the provider's recycle bin | A rename inside the sync root syncs as a move without downloads, stays restorable, and keeps the rule that only the purge deletes |
| "Free up space" changes only the pin state (`attrib -p +u`), and only for files in sync | It is what Explorer does; files not uploaded yet would lose their only copy |
| Google Drive for desktop is skipped by default | It is a virtual drive, not a Cloud Files API sync root |

## Photos

| Decision | Reason |
|---|---|
| The photo review uses names, sizes, dates, metadata and Windows thumbnails only | No downloads, and no decoder of our own for untrusted images |
| Local exact duplicates need a full BLAKE3 hash; cloud-only matches are "almost certainly identical" | Honest about certainty |
| The review grid starts with nothing selected; "select all but the keeper" is one action | S1, and still fast |
| The best-shot suggestion uses sharpness, then resolution, then size, and says thumbnails limit accuracy | Useful without overclaiming |
| Video: exact duplicates in the MVP; similar videos (ffmpeg) in Next | ffmpeg is a large dependency that needs its own review |

## Measures and units

| Decision | Reason |
|---|---|
| **Size on disk** is the default measure; logical size is a switch | It is what the drive actually loses |
| **Binary units labelled "GB"**, as Explorer does | The numbers match what users see in Explorer and Settings |
| Items under 0.5% of the current view are grouped into "N smaller items" | Keeps the map readable and the layout fast |
| Cell areas are within 3% of their share after settling | Accurate enough to trust, and the layout converges in a fraction of a second |
| An unknown size is `null` or "unknown", never 0 | A 0 would mislead |

## Privacy and network

| Decision | Reason |
|---|---|
| **The MVP makes no network calls at all** (S8) | The strongest privacy promise is one that is easy to verify |
| The opt-in update check is in Next | Keeps the MVP fully offline |
| **No telemetry, ever** | Trust; a disk tool sees the whole PC |
| Exports and CLI output replace the profile path with `%USERPROFILE%` (S9) | Shared output does not leak the user name |
| Cloud AI only after explicit consent; the local model is the default | Privacy first |
| Contact for the Code of Conduct and security reports goes **through GitHub only** (private vulnerability reporting and the maintainer's profile) | No personal email address in the repository |

## Engineering

| Decision | Reason |
|---|---|
| **Tauri 2, a Rust core and a React/TypeScript UI** | Small binary, memory-safe core, rich animated UI ([Tech stack](21-tech-stack.md)) |
| The engineering standards are public in `AGENTS.md` and apply to every contributor and agent | One copy of the rules, enforced by tests, lints and CI |
| Test-driven development for every Medium or higher change | The tests prove the safety invariants |
| Test data is always generated, never real photos or scans | Privacy, and reproducible tests |
| `unsafe` only in the Windows FFI module | Keeps the unsafe surface small and reviewable |
| Enumeration through `GetFileInformationByHandleEx` with `FileIdExtdDirectoryInfo`, falling back to `FileIdBothDirectoryInfo` on FAT and exFAT | Sizes, 128-bit file IDs and reparse tags in one batched call per folder, through a documented Win32 API |
| The minimum Rust version is 1.97 | The patched releases of `quick-xml` and `time`, needed to clear RustSec advisories, require it |
| Known-location rule IDs are kebab-case (`node-modules`) and match their text keys | One ID links the rule, its text, the manifest and the CLI output |
| The generic Review case also covers unmatched files of 1 GiB or more | A large unknown file is exactly what the user asks about; it gets context without a guess that it is safe |
| A known location is added only when its path and behaviour are confirmed in official documentation | A wrong explanation or tier would break trust; unconfirmed entries wait for a contributor who can verify them |
| The overview screen stays mounted and is hidden when another screen shows | Switching back is instant and the cell map keeps its settled layout (P2) |
| The performance runs use a production build with a fake core that generates a tree of a set size | The numbers measure the UI itself, repeatable on any PC and on CI, without a real disk |
| The UI asks for views of the tree, never the whole tree | Memory and IPC stay bounded |
| Canvas 2D for the cell map; WebGL only if the frame-rate target fails | Simplest first |
| Our own power-diagram layout | No maintained library does weighted Voronoi treemaps with per-frame relaxation |
| **Performance budgets P1–P14** are hard limits: a visible response within 50 ms, a screen switch painted within 100 ms, 60 fps, no UI-thread task over 50 ms ([Performance](25-performance.md)) | They follow the perception limits for "instant" (100 ms) and "smooth" (16.7 ms per frame); a premium, animated app that lags feels worse than a plain one |
| Performance is checked by benchmarks and scripted interactions against a committed baseline; CI fails above a budget or at more than 20% slower, locally a warning shows at 10% | A slowdown is caught in the change that causes it, while the developer still has the context; 20% absorbs the noise of shared CI runners |
| UI timings are measured in Chromium with a fake core at 4× CPU throttling | Repeatable on CI; the throttling approximates a mid-range PC, and a manual check confirms it in the real app before a release |

## Docs

| Decision | Reason |
|---|---|
| A flat `docs/` with numbered files; the first digit is the cluster; never renumber | Lists in reading order, and links never break |
| Each doc has a breadcrumb and "Related in this cluster"; the hub, `llms.txt` and the README list the same docs | Topical clusters that people and AI tools can follow; CI checks it |
| No version numbers in docs or the UI; the roadmap uses Now, Next and Later | Docs stay true without bumping numbers |
| Docs describe the current state, never the history | Git holds the history |

## Distribution

| Decision | Reason |
|---|---|
| GitHub Releases with a per-user installer and a portable exe, unsigned for now | No admin to install; signing comes with SignPath Foundation in Next |
| A winget manifest from day one with the id **`KeesVanWanrooij.Unhoard`**; submitting it needs the maintainer | winget is how power users install; the id follows the Publisher.App convention |
| The release workflow runs only by hand | Publishing is outward-facing and needs the maintainer |

## Dependencies

Every dependency has its reason here. The build phase adds a row before it adds a crate or
package.

| Dependency | Reason |
|---|---|
| `tauri` | The application shell |
| `windows` | Official Rust bindings for Win32, NT and WinRT |
| `rayon` | A proven work-stealing pool for the parallel scan |
| `serde`, `serde_json`, `toml` | Manifests, settings, rule files and CLI JSON |
| `thiserror`, `anyhow` | Typed errors in libraries, context at the binary edge |
| `clap` | CLI arguments |
| `globset` | Matches all parent globs of the known-locations rules in one pass; part of ripgrep, widely used and maintained |
| `blake3` | Fast, chunked content hashing |
| `tempfile` (dev) | Generated test trees |
| `react`, `react-dom`, `typescript`, `vite`, `@vitejs/plugin-react` | The UI |
| `@tauri-apps/api`, `@tauri-apps/cli` | IPC from the UI to the Rust core, and the build and bundle tool |
| `@fontsource/rajdhani`, `@fontsource/jetbrains-mono` | The bundled fonts as local files, so the app loads nothing from the network |
| `i18next`, `react-i18next` | Translations with plurals |
| `zustand` | Small state stores |
| `vitest`, `jsdom`, `@testing-library/react` (dev) | UI unit tests in a simulated DOM |
| `eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-react-hooks`, `globals`, `prettier` (dev) | Lints and formatting; `typescript-eslint` supplies the type-aware rules such as no floating promises |
| `@types/react`, `@types/react-dom`, `@types/node` (dev) | Types for React and for the Node-based perf scripts |
| `criterion` (dev) | Statistically sound Rust benchmarks for the performance budgets |
| `@playwright/test` (dev) | Drives the UI to measure interaction budgets; widely used and maintained by Microsoft |
| GitHub Actions: `actions/checkout`, `actions/setup-node`, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `EmbarkStudios/cargo-deny-action`, `gitleaks/gitleaks-action` | CI, each pinned to a full commit SHA |

## Open questions

Questions that wait for the maintainer are listed here, and text in other docs that
depends on one is marked "(pending)". There are no open questions at the moment.

---

## Related in this cluster

- [Architecture](20-architecture.md): the structure these decisions shape.
- [Tech stack](21-tech-stack.md): the stack choices in detail.
- [Development](22-development.md): the workflow these decisions set.
- [Translations](23-translations.md): the language and unit decisions in practice.

Back to the [docs hub](README.md).
