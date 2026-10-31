[Unhoard](../README.md) › [Docs](README.md) › Engineering › Development

# Development

How to set up, build and test Unhoard, how we write tests first, how code is commented,
and how a release is made. The full engineering standards (risk classes, security,
safe file operations, errors, architecture, supply chain and the Definition of done) are
in [AGENTS.md](../AGENTS.md).

## Setup

You need Windows 10 or 11 and:

- Rust (stable) through `rustup`, with the `x86_64-pc-windows-msvc` target
- the Visual Studio Build Tools with the "Desktop development with C++" workload
- Node.js (current LTS) and npm
- WebView2 (included in Windows 11)
- `cargo-deny`: `cargo install cargo-deny --locked`

Then:

```powershell
git clone https://github.com/keesvanwanrooij/unhoard.git
cd unhoard
npm ci
```

## Build and run

| Command | What it does |
|---|---|
| `npm run tauri dev` | Runs the app with hot reload |
| `npm run tauri build` | Builds the installer and the portable exe |
| `cargo run -p unhoard-cli -- scan <path> --json` | Runs the CLI |

## Checks

Every change runs these before it is done. CI runs them on `windows-latest`.

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
npm run typecheck
npm run lint
npm test
npm run build
npm audit --audit-level=high
```

If a check does not exist yet, the report says so. It is never reported as passed.

Changes that touch the UI, the core's hot paths or IPC also run the performance checks.
Run them before you start and after each step, so a slowdown shows up while you work:

```powershell
cargo bench -p unhoard-core
npm run perf
npm run perf:check
```

The budgets and the regression rule are in [Performance](25-performance.md).

## Risk classes

Classify every change before you start, and name the class in the commit body or pull
request (`Risk: medium`). The class sets the minimum rigour: Low (layout, copy), Medium
(scanning, rules, tiers, i18n), High (anything that moves, restores or frees up space)
and Critical (the purge, bulk operations, anything that touches S1–S10). The table with
the required steps per class is in [AGENTS.md](../AGENTS.md).

## Test-driven development

For every Medium or higher change:

1. **Red.** Write a test that describes the behaviour and run it. See it **fail**. That
   proves the test can catch the problem.
2. **Green.** Write the code until the test passes.
3. **Refactor** while it stays green.

Rules:

- A bug fix starts with a test that reproduces the bug.
- In a build phase, the test list in [MVP scope](05-mvp-scope.md#tests-that-come-first)
  comes first.
- Test names carry the feature or invariant ID: `s5_manifest_written_before_move`,
  `f4_most_specific_rule_wins`.
- Rust uses `cargo test`; TypeScript uses Vitest.
- **Test data is generated** in temp folders (`tempfile`): files, folders, hard links,
  junctions, long paths, Unicode names and images with known blur or noise. Never a real
  disk, a real profile, real photos or a real scan.
- Core tests use the **fake platform layer**, which simulates placeholders, locked files,
  access denied, vanishing files, a full disk and a clock.
- Windows integration tests run against a temp folder on the CI runner.
- High and Critical changes add failure and attack tests: stale IDs, swapped files, links
  pointing out of scope, and a dry run that provably writes nothing.
- Purely visual work is checked by running the app, not by unit tests.

## Manual checks

Some behaviour depends on a real OneDrive client and cannot run in CI. Before a release,
run this checklist on a real OneDrive personal folder that holds **only test files**
created for the purpose:

1. A scan shows online-only files with 0 bytes on disk, and their allocation size stays 0
   afterwards.
2. Thumbnails of online-only images appear without the files becoming locally available.
3. Video duration and resolution can be read from placeholders without hydration.
4. "Free up space" on a locally available folder makes it online-only, and Unhoard reports
   the space that was really freed.
5. A file that is not yet uploaded is not offered for "free up space".
6. A move into the cloud quarantine leaves online-only files online-only, and OneDrive
   shows the move on the web.
7. A restore from the cloud quarantine puts the files back without a download.
8. The cloud purge sends the batch to the OneDrive recycle bin.

Also check by hand: the UAC flow of "Reveal hidden space", reduced motion, Dutch and
English, the installer without admin, and the budgets P2 (screen switch), P5 (zoom) and
P10 (frame time) in the real app on a mid-range PC ([Performance](25-performance.md)).

## Comment standard

- Every file starts with a header comment that says what it is for.
- Every function and type has a doc comment (`///` in Rust, TSDoc in TypeScript).
- "Why" comments where the reason is not obvious. No line-by-line narration.
- Comments say what the code does and why, never what it used to do.
- Each `unsafe` block carries a `// SAFETY:` comment.
- No dead or commented-out code.

```rust
//! Known-locations matcher: picks the rule that explains a scanned item.

/// Returns the winning rule for `item`: the longest matched path, then the strictest
/// tier, so a "Keep" item is never shown as safer than it is.
pub fn best_match<'a>(item: &Item, rules: &'a [Rule]) -> Option<&'a Rule> { … }
```

## Commits and pull requests

- [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`,
  `test:`, `refactor:`, `chore:`).
- Every commit is signed off for the DCO: `git commit -s`.
- The commit body names the risk class and the F or S IDs it touches.
- Changes land on `main` through a pull request with green CI. Critical changes wait for
  the maintainer's review.

Details: [CONTRIBUTING.md](../CONTRIBUTING.md).

## Releases

- The release workflow runs only when started by hand. It builds the installer, the
  portable exe and SHA-256 checksums.
- Publishing a GitHub Release and submitting the winget manifest need the maintainer.
- GitHub generates the release notes. The docs and the UI show no version numbers;
  Settings › About shows the build identifier for bug reports.

---

## Related in this cluster

- [Architecture](20-architecture.md): what you are building.
- [Tech stack](21-tech-stack.md): the tools used here.
- [Translations](23-translations.md): adding and checking text.
- [Decisions](24-decisions.md): why the rules are what they are.
- [Performance](25-performance.md): the speed budgets and the regression check.

Back to the [docs hub](README.md).
