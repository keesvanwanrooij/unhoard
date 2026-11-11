[Unhoard](../README.md) › [Docs](README.md) › Features › Known locations

# Known locations

The known-locations library turns "a 64 GB file called `ext4.vhdx`" into "this is your
WSL Linux disk; it does not shrink by itself; here is how to compact it". Each entry gives
an explanation, a risk tier with its reason, and one action. Next to those full rules,
**definitions** say what a known folder is without an opinion about removing it, and
every item that has neither still gets an explanation built from its facts. The library
is community-curated: adding an entry needs no Rust. Acceptance criteria: F4 in
[MVP scope](05-mvp-scope.md).

## How the library works

The library is a **switch-case kept as data**. One small matcher in the Rust core reads a
table of rules, so the code holds no branch per location. Every rule has its own
generated fixture test.

### Rule files

Rules live in TOML files in `data/known-locations/`, one file per group (`windows.toml`,
`caches.toml`, `containers.toml`, `games.toml`, `developer.toml`, `user.toml`,
`apps.toml`), plus the definitions in `definitions-windows.toml`, `definitions-user.toml`
and `definitions-apps.toml`.

```toml
[[rule]]
id = "node-modules"
parent = "**"
name = "node_modules"
kind = "dir"
sibling = "package.json"
category = "developer"
tier = "likelySafe"
action = "quarantine"
example = "C:\\code\\app\\node_modules"
nearMiss = "C:\\code\\lib\\node_modules"
```

| Field | Meaning |
|---|---|
| `id` | Stable kebab-case ID, used in text keys, the manifest and the CLI output |
| `parent` | Glob of the folder that holds the match, with environment variables (`%LOCALAPPDATA%`; `%DOWNLOADS%` is the Downloads Known Folder) |
| `name` | Glob of the item's name, for example `*.{exe,msi}` |
| `kind` | `dir`, `file` or `any` |
| `sibling` | Optional glob of a file or folder that must sit next to the match, so `target` is only a Rust build folder next to a `Cargo.toml` |
| `stemSibling` | Optional: a folder named like the match without its extension must sit next to it (an extracted archive) |
| `minAgeDays`, `minBytes` | Optional limits, for example installers older than 30 days |
| `category` | The kind of content, which sets the map colour of the item and everything inside it ([Storage map](10-storage-map.md)). A definition may leave it out, so its contents keep the colour of their own type and place |
| `tier` | `safe`, `likelySafe`, `review` or `keep` ([Cleanup and safety](11-cleanup-safety.md#risk-tiers)). A definition has none, or `keep` |
| `action` | `quarantine`, `freeUpSpace`, `windowsTool:<name>`, `uninstaller`, `showInExplorer` or `explainOnly`. A definition has none |
| `define` | `true` makes the rule a definition ([Definitions](#definitions)) |
| `requiresAdmin` | The action needs admin, so Unhoard explains and points to the Windows tool |
| `docs` | Official pages the explanation is based on |
| `exclude` | Optional list of folders, with environment variables, where the rule never matches, in the folder itself or anywhere below it. The project rules use it to skip app folders, hidden folders in the profile and nested `node_modules` |
| `example`, `nearMiss` | A path that must match and a similar path that must not; the tests generate both |

The text is not in the rule. It lives in the locale files under
`knownLocations.<id>.title`, `.explain`, `.reason` and `.action`, in English and Dutch; a
definition has only `.title` and `.explain`. A rule without both languages fails CI
(AC4.4).

### Testing a rule

For every rule, the test suite generates a temp-folder fixture that must match, and a
near miss that must not: `node_modules` next to `package.json` matches, `node_modules`
without it does not (T-4.1).

## Precedence and the default case

- **A full rule wins over a definition** for the same item.
- **Most specific wins.** When several rules match one item, the rule with the longest
  matched path wins.
- **On a tie, the strictest tier wins:** Keep over Review over Likely safe over Safe. A
  Keep item is never shown as safer than it is (T-4.4).
- **Protected paths** ([Cleanup and safety](11-cleanup-safety.md#protected-paths)) never
  offer a move, whatever the rule says (T-4.3).
- **The default case:** an item that matches no rule is explained from its facts
  ([Explanations for everything else](#explanations-for-everything-else)). When it is
  1 GiB or larger its tier is **Review**, and its only actions are "Show in Explorer" and
  "Move to quarantine" (T-4.5).

## Definitions

A definition says what a known folder or file is: "the registry", "your Firefox profile",
"the data folder of Discord". It is a rule with `define = true`:

```toml
[[rule]]
id = "def-roaming-thunderbird"
define = true
parent = "%APPDATA%"
name = "Thunderbird"
kind = "dir"
category = "apps"
example = "C:\\Users\\u\\AppData\\Roaming\\Thunderbird"
nearMiss = "C:\\Users\\u\\AppData\\Roaming\\x-Thunderbird"
```

- It has a title and an explanation, and no action.
- It has no tier, so it gives no opinion about removing the item, or `tier = "keep"` for
  what Windows or a program needs. An item with a Keep definition is never offered for a
  move.
- It is never listed in Suggestions or the priority matrix (T-4.7).
- The item keeps the actions of an item without a rule: Show in Explorer and, where the
  path is not protected, Move to quarantine.

The definitions cover:

| File | What it defines |
|---|---|
| `definitions-windows.toml` | The top of a drive (`Windows`, `Program Files`, `ProgramData`, `Users`, upgrade and recovery folders, unpacked driver installers) and the folders inside `Windows` and `System32` |
| `definitions-user.toml` | The user's folders (Desktop, Documents, Pictures, …, also under their Dutch OneDrive names), what apps keep inside Documents, Pictures and Videos, developer folders in the profile (`.ssh`, `.cargo`, `.docker`, …), the three parts of `AppData`, and the shared folders in `Program Files` and `ProgramData` |
| `definitions-apps.toml` | App folders in `AppData` (Local, Roaming, LocalLow) for common apps, the Microsoft folders there, well-known Microsoft Store apps in `Packages`, and the parts many apps share anywhere in `AppData`: cache, compiled-code cache, logs, crash reports, built-in browser data, updates, backups and extensions |

A definition rests on what is commonly known about the folder; it is corrected when
someone shows it wrong. A full rule, which carries a tier and an action, needs its path
and behaviour confirmed in official documentation.

## Explanations for everything else

An item without a rule is explained from facts the scan already has; nothing extra is
read (`crates/core/src/explain.rs`):

| Fact | Where it comes from | Example in the text |
|---|---|---|
| Place | The path: Downloads, Desktop, Documents, Pictures, Videos, Music, a cloud folder, `AppData`, programs installed for the user only, Microsoft Store app data, Temp, Program Files, ProgramData, Windows, the profile, another user's profile, the Recycle Bin, the quarantine, or elsewhere on a drive | "It is in your Downloads folder" |
| Owner | The folder name right below `AppData\Local`, `AppData\Roaming`, `AppData\LocalLow`, `Program Files`, `ProgramData` or `Users`; for a Store app the package name without its publisher ID | "It belongs to the data the app “Vendor” keeps for your account" |
| File type | The extension, in 31 types: video, photo, raw photo, audio, archive, disk image, virtual disk, installer, program, library, document, spreadsheet, presentation, PDF, e-book, text, source code, data, database, log, backup, crash dump, AI model, game data, font, mail, design project, unfinished download, temporary file, shortcut and system file | "“linux.iso” is a disk image." plus one sentence about that type |
| Contents of a folder | The file count, and the kind of content that fills at least half of its bytes on disk | "… a folder with 214 files. 98% of its space goes to photos and videos." |

The UI builds the sentences (`src/explain.ts`), with the item's own name in them, so two
items never share an explanation. An owner's own folder gets its own sentence ("the data
folder of an app or maker with that name"). The details panel shows the full explanation;
the map tooltip and the list show one line, such as "Video in Downloads" (T-4.8).

## MVP entries

Paths marked (verify) are confirmed with a fixture and an official source during the
build. An entry that cannot be confirmed is left out.

### Windows

| Entry | Tier | What Unhoard says and does |
|---|---|---|
| `Windows.old` | Likely safe | The previous Windows installation, kept for rollback. Points to Disk Cleanup › "Previous Windows installation(s)" |
| `hiberfil.sys` | Keep | The hibernation file. Explains `powercfg /h /type reduced` or `powercfg /h off`, which needs admin and also turns off Fast Startup. Explain only |
| `pagefile.sys`, `swapfile.sys` | Keep | Virtual memory. Never disable it. Explain only |
| `WinSxS` | Keep | The component store. Explorer overstates its size because of hard links. Points to Disk Cleanup or `DISM /Online /Cleanup-Image /StartComponentCleanup`. Explain only |
| Temp (`%TEMP%`): each item at least a day old, on its own | Safe; items in use are skipped by the move |
| `%WINDIR%\Temp` | Safe; points to Disk Cleanup |
| WSL's Remote Desktop traces (`%TEMP%\DiagOutputDir\RdClientAutoTrace`) | Safe; explains `wsl --shutdown` first and `guiApplications=false` in `.wslconfig` |
| `SoftwareDistribution\Download` | Likely safe | Windows Update downloads. Needs admin; points to Disk Cleanup |
| Delivery Optimization cache | Likely safe | Update files shared with other PCs. Points to Disk Cleanup |
| Crash dumps (`MEMORY.DMP`, `Minidump`, `%LOCALAPPDATA%\CrashDumps`) | Likely safe | Only useful for debugging a crash |
| `Windows\Installer` | Keep | Needed to repair and uninstall programs. Explain only |
| System Volume Information | Keep | Restore points and shadow copies. Points to System Protection settings |

### Caches

Unhoard tells you to close the app first, and never touches profile data (bookmarks,
passwords, history).

| Entry | Tier |
|---|---|
| Chrome, Edge and Brave `Cache`, `Code Cache`, `GPUCache` | Safe |
| Firefox `cache2` | Safe |
| Shader caches: `D3DSCache`, NVIDIA `DXCache` and `GLCache`, AMD `DxCache`, Steam `shadercache` | Safe |
| Thumbnail cache | Safe; points to Disk Cleanup |
| Discord cache | Safe |
| Adobe Media Cache | Safe |

### Large containers

All are **Review**: they hold your data in one big file.

| Entry | What Unhoard explains |
|---|---|
| WSL `ext4.vhdx` (`%LOCALAPPDATA%\Packages\*\LocalState` or `%LOCALAPPDATA%\wsl\{guid}`) | The Linux disk grows but never shrinks by itself; how to compact it |
| Docker Desktop `docker_data.vhdx` or `ext4.vhdx` | `docker system prune` and compacting |
| Hyper-V, VirtualBox and VMware virtual machines | Remove VMs from their own manager |
| Android SDK and emulator images (AVDs) | Remove through Android Studio's SDK manager |
| Hugging Face and Ollama model caches | Remove models through the tool that downloaded them |

### Game launchers

Steam, Epic, Xbox (`C:\XboxGames`), EA, Ubisoft and Battle.net. **An installed game is
never quarantined:** Unhoard sends you to the launcher's uninstall or to Settings › Apps,
so the launcher stays consistent. Tier Review. A Steam game folder without a Steam
manifest is a leftover of an uninstalled game ([App adapters](#app-adapters)).

### Developer clutter

| Entry | Tier |
|---|---|
| npm, pnpm and Yarn caches | Safe |
| pip cache | Safe |
| cargo `registry` and `git` | Safe |
| NuGet `packages` and `v3-cache` | Safe |
| Gradle caches, Maven `.m2` | Safe |
| `node_modules` next to `package.json` | Likely safe |
| Rust `target` next to `Cargo.toml` | Likely safe |
| `.venv` | Likely safe |
| .NET `bin` and `obj` next to a `*.csproj` | Likely safe |
| `ProgramData\Package Cache` | Keep: needed to repair or uninstall |

### User leftovers

| Entry | Tier |
|---|---|
| Recycle Bin (size through `SHQueryRecycleBinW`) | Likely safe; points to emptying it |
| Downloads: installers older than 30 days | Likely safe |
| Downloads: ISO files | Review |
| Downloads: ZIP files with an extracted sibling folder | Likely safe |
| Phone backups in `Apple\MobileSync\Backup` | Review |

### App data and caches

Caches an app rebuilds are Safe; what the user downloaded on purpose is Review and points
to the app's own way of removing it.

| Entry | Tier |
|---|---|
| VS Code and Cursor caches (`Cache`, `CachedData`, `CachedExtensionVSIXs`, `Code Cache`, `GPUCache`) | Safe |
| Slack caches; Steam's web cache (`htmlcache`); the Epic Games Launcher's `webcache` | Safe |
| The new Microsoft Teams cache (`MSTeams_8wekyb3d8bbwe\LocalCache\Microsoft\MSTeams`) | Likely safe |
| JetBrains IDE `caches`; the Unity cache | Safe |
| Go module, Bun, uv and Poetry caches | Safe |
| conda package caches (`miniconda3\pkgs`, `anaconda3\pkgs`) | Likely safe; points to `conda clean --all` |
| Ollama's downloaded updates (`Ollama\updates*`) | Safe |
| PyTorch Hub models | Likely safe |
| Whisper speech models (`.cache\whisper`) | Review |
| LM Studio models | Review; points to LM Studio |
| Rust toolchains (`.rustup\toolchains`) | Review; points to `rustup toolchain uninstall` |
| Outlook's offline mailbox (`*.ost`) | Review; points to Outlook's "mail to keep offline" |

## Installed programs

Rules explain folders that apps fill. The install folders of the apps themselves come
from Windows' own list of installed programs, the same list Settings › Apps shows (the
`Uninstall` registry keys, per machine and per user). For a folder that is a program's
install location, Unhoard shows the program's name, publisher, install date and size, and
offers its uninstall through Settings › Apps. This covers nearly every installed program
without a rule.

## App adapters

Some apps keep their own records of what is installed. An adapter reads those records,
so Unhoard can say more than a folder name. Adapters only read; actions stay reversible
or go through the app itself.

| App | What it reads | What Unhoard adds |
|---|---|---|
| Steam | `libraryfolders.vdf` and each library's `appmanifest_*.acf` | Per game: name, size and last played; "uninstall in Steam" opens Steam's own dialog, "Open Steam" its library. A folder in `steamapps\common` without a manifest is a **leftover** of an uninstalled game: Likely safe, with Show in Explorer |
| Ollama | The model manifests in `.ollama\models\manifests` (or `OLLAMA_MODELS`) | Per model: name, size, download date and the data it shares with other models. A model moves to the quarantine as a whole (its manifest plus the data no other model uses), or you copy `ollama rm <model>`. "Open Ollama" starts the app |
| Docker Desktop | After a click, `docker system df` from Docker's install folder (per user or per machine) | What fills the virtual disk (images, containers, volumes, build cache) and how much can be reclaimed; the commands to clean it in the order that frees most (build cache, stopped containers, unused images, then volumes with care); how to compact the disk afterwards; "Open Docker Desktop"; Show in Explorer for the disk file |

Unhoard runs a tool like `docker` only after a click, from the app's own install folder,
with fixed read-only arguments, no shell and a time limit
([Architecture](20-architecture.md#local-tools)).

## Grouped suggestions

Suggestions with the same rule form one row with their count and total size, for example
every browser profile's cache, or every `node_modules` folder. The row expands to the
single items; one preview covers the whole group, and nothing is pre-selected (S1).

## Adding a rule

1. Add the rule to the right TOML file.
2. Add the four text keys in English and Dutch.
3. Add the fixture and the near miss.
4. Link the official source in `docs`.

See [CONTRIBUTING.md](../CONTRIBUTING.md). A rule change is a Medium-risk change (it sets
risk tiers), so it is test-first.

---

## Related in this cluster

- [Cleanup and safety](11-cleanup-safety.md): the tiers, protected paths and the quarantine.
- [Storage map](10-storage-map.md): where the explanations show up.
- [Cloud storage](13-cloud-storage.md): cloud items have their own actions.
- [Optimise](16-optimise.md): the check-up links into these entries.

Back to the [docs hub](README.md).
