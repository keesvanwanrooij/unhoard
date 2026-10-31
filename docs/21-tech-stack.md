[Unhoard](../README.md) › [Docs](README.md) › Engineering › Tech stack

# Tech stack

The choices behind Unhoard and the alternatives we rejected. Every dependency also has
its reason in [Decisions](24-decisions.md), and new ones follow the dependency checklist
in [AGENTS.md](../AGENTS.md).

## Application shell: Tauri 2

Tauri gives a small native binary with a Rust backend and a system webview (WebView2) for
the UI. The Rust side is where all safety decisions live; the webview only renders.

| Rejected | Why |
|---|---|
| Electron | A bundled Chromium makes the download and memory use several times larger |
| WinUI 3 or WPF on .NET | Windows-only, and rich web-style motion is harder to build |
| egui or Slint | Immediate or declarative native UIs, but less freedom for an animated, designed stage |
| Flutter | A second language and UI toolkit next to Rust, with weaker Windows file-system integration |

## UI: React and TypeScript

React has the largest pool of contributors, which matters for an open-source project.

| Rejected | Why |
|---|---|
| Svelte, Solid | Smaller contributor pool; no feature we need that React lacks |

### UI libraries

| Library | Use |
|---|---|
| Vite | Dev server and build |
| Plain CSS with design tokens (CSS variables) and CSS modules | Styling, theming and the design direction's palette |
| Motion (formerly Framer Motion) | UI transitions in the shell |
| Zustand | Small global state stores |
| i18next with react-i18next | English and Dutch, ICU-style plurals |
| d3-hierarchy | The squarified treemap |
| Our own code | The power-diagram cell map and the polygon morph |
| Rajdhani and JetBrains Mono (SIL Open Font License), bundled | Headings, stage labels, numbers and paths; body text uses Segoe UI Variable from Windows |

## Core: Rust

| Crate | Use |
|---|---|
| `windows` (windows-rs) | Win32, NT and WinRT calls, only inside the platform FFI module |
| `rayon` | The work-stealing scan pool |
| `serde`, `serde_json`, `toml` | Manifests, settings, rule files and CLI JSON |
| `thiserror` | Typed errors in library crates |
| `anyhow` | Error handling at the binary edge (CLI, Tauri entry) |
| `clap` | CLI arguments |
| `tracing` | Local diagnostic logging, off unless switched on |
| `blake3` | Full content hashes for local exact duplicates |
| Our own code | dHash and pHash, the Laplacian sharpness score |

## Tests and checks

| Tool | Checks |
|---|---|
| `cargo test` | Rust unit and integration tests |
| Vitest with Testing Library | UI unit tests |
| `cargo clippy` with `-D warnings` | Rust lints, including `unwrap_used`, `expect_used`, `panic` and `undocumented_unsafe_blocks` |
| `rustfmt`, Prettier | Formatting |
| ESLint | TypeScript lints: no floating promises, no empty `catch`, no hard-coded UI text |
| TypeScript `strict` | Types, no `any` |
| `cargo deny` | Licenses compatible with GPL-3.0, RustSec advisories, bans and sources |
| `npm audit --audit-level=high` | npm advisories |
| gitleaks | Secrets in the history |
| criterion | Rust core benchmarks with statistics ([Performance](25-performance.md)) |
| Vitest bench | Benchmarks of the layout functions, built into Vitest |
| Playwright | Scripted interaction timings of the real UI in Chromium, with CPU throttling |

## Packaging

- Tauri's NSIS bundler in **per-user mode**, so installing needs no admin.
- A portable exe next to the installer.
- The installer bootstraps WebView2 when it is missing. The portable exe explains how to
  install it.
- A winget manifest in `packaging/winget/`, with the package id `KeesVanWanrooij.Unhoard`.

## Not used

- No HTTP client and no Tauri HTTP plugin in the MVP (S8).
- No crash-reporting or analytics SDK, ever ([Privacy](30-privacy.md)).
- No image decoder for full images: thumbnails come from Windows.

---

## Related in this cluster

- [Architecture](20-architecture.md): where each library sits.
- [Development](22-development.md): how to install and run the tools.
- [Decisions](24-decisions.md): the reason for every dependency.
- [Translations](23-translations.md): how i18next is used.

Back to the [docs hub](README.md).
