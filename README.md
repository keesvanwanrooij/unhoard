<div align="center">

# Unhoard

**See where your disk space went, and get it back safely.**

[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
![Platform: Windows 10 and 11](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078d4.svg)
[![CI](https://github.com/keesvanwanrooij/unhoard/actions/workflows/ci.yml/badge.svg)](https://github.com/keesvanwanrooij/unhoard/actions/workflows/ci.yml)

</div>

> **Status: early development.** The first version is built and being tested. There is
> no release yet; you can run it from source, see [Start the app](#start-the-app).

Unhoard is a free, open-source Windows app for everyone whose disk is full and who does
not know why. It maps your whole drive as a living cell map, explains what each big item
is and how risky it is to remove, understands OneDrive files that live only in the cloud,
and helps you sort years of phone photos. Nothing is deleted without your click, and
everything you remove can be restored for 30 days.

## Why another disk analyzer?

Fast disk maps already exist. They show *how big* things are, but not *what* they are,
whether you can remove them, or what happens in the cloud. Built-in Windows tools explain
a little, but go one folder at a time. Unhoard joins the two: a full map of the drive,
with an explanation and a safe next step for every big item.

## Features

**A living map of your drive.** Your disk as organic cells that grow while the scan runs
and breathe at rest, coloured by kind of content. Prefer something plain? Switch to a
simple treemap. A sorted list next to the map shows the same data.

**Hidden space, explained.** The space Windows keeps out of sight (restore points, other
accounts, protected folders) shows as its own cell, with what usually hides there. One
click, with your permission, reveals more through a read-only scan.

**An explanation for every big item.** A curated library knows WSL disks, game libraries,
browser caches, `node_modules`, `Windows.old`, the page file and many more. Each comes
with a risk tier (Safe, Likely safe, Review, Keep), the reason, and the right action.

**A safety net for every removal.** Items go to a quarantine on the same drive, with a
record of where each came from. Restore puts them back exactly. After 30 days, and a
warning first, Unhoard empties it.

**OneDrive without downloads.** See how much of your cloud library is on the disk and
free up local space without downloading or deleting anything in the cloud.

**Photo review in seconds.** Groups of duplicates, near duplicates, bursts, chat media and
screenshots, reviewed one grid at a time, with a suggested best shot. Online-only photos
are never downloaded.

**A companion, not popups.** A small guide character explains what you see in speech
bubbles that never block your work.

**English and Dutch.** Unhoard follows your Windows language.

**A CLI.** `unhoard scan <path> --json` for scripts and power users.

## What it does not do

- It never selects, moves or deletes anything by itself.
- It does not clean the registry, "free" RAM, tweak services or promise a faster PC.
- It does not delete Windows system files. For those, it points you to the Windows tools
  that do it safely.
- It does not send anything anywhere.

## Privacy

No telemetry, ever. The first version opens no network connection at all. Scan results
stay in memory, and anything you export replaces your user folder with `%USERPROFILE%`.
See [Privacy](docs/30-privacy.md).

## Start the app

**From a release.** Once there is one, install Unhoard from an official source under
[Download](#download) and start it from the Start menu, or run the portable exe.

**From source.** You need Windows 10 or 11, Rust, the Visual Studio Build Tools with
"Desktop development with C++", and Node.js; the full list is in
[Development](docs/22-development.md#setup). Then, in PowerShell:

```powershell
git clone https://github.com/keesvanwanrooij/unhoard.git
cd unhoard
npm ci
npm run tauri dev
```

The first start compiles the Rust code and takes a few minutes; then the Unhoard window
opens. To build your own installer and portable exe:

```powershell
npm run tauri build
```

The installer, which needs no admin, is
`target\release\bundle\nsis\Unhoard_<version>_x64-setup.exe`. The portable exe is
`target\release\unhoard-app.exe`.

**Only the interface**, with generated example data and no disk access, runs in a browser
after `npm ci`:

```powershell
npx vite --mode fake
```

Then open <http://localhost:5173>.

**The CLI:**

```powershell
cargo run --release -p unhoard-cli -- scan "$env:USERPROFILE\Downloads" --json
```

## Download

There is no release yet. When there is, these are the only official sources:

| Source | How |
|---|---|
| GitHub Releases | Installer (no admin needed) and portable exe, with SHA-256 checksums |
| winget | `winget install KeesVanWanrooij.Unhoard` |

Unhoard downloaded from anywhere else may be modified. Check the checksum when in doubt.

<details>
<summary>Windows SmartScreen warns about the installer. Why?</summary>

The first releases are not code-signed yet, so SmartScreen does not know them. Signing
through the SignPath Foundation is on the [roadmap](docs/06-roadmap.md). Until then,
download only from the official sources above and compare the checksum.
</details>

<details>
<summary>Is it safe to use on my PC?</summary>

Unhoard only reads while it scans. It removes nothing without your click, never touches
Windows or program folders, and keeps every removed item restorable for 30 days. The
safety rules are tested; see [Cleanup and safety](docs/11-cleanup-safety.md) and the
[threat model](docs/32-threat-model.md).
</details>

<details>
<summary>Does it need administrator rights?</summary>

No. The only exception is the optional "Reveal hidden space" scan, which asks through
the normal Windows prompt and only reads.
</details>

<details>
<summary>Does it use the network?</summary>

No. The first version has no network code at all. Later features that need the network,
such as an update check, are off until you switch them on.
</details>

## How it works

A Rust core scans the drive in parallel and decides everything that matters for safety.
A Tauri shell exposes a small set of commands to a React interface that only renders.
Windows calls sit behind a platform layer, so the core is tested without a real disk. See
[Architecture](docs/20-architecture.md).

## Documentation

The [docs hub](docs/README.md) lists every document by cluster.

**Product:**
[Vision](docs/01-vision.md) ·
[Target users](docs/02-target-users.md) ·
[Market landscape](docs/03-market-landscape.md) ·
[Competitive advantage](docs/04-competitive-advantage.md) ·
[MVP scope](docs/05-mvp-scope.md) ·
[Roadmap](docs/06-roadmap.md)

**Features:**
[Storage map](docs/10-storage-map.md) ·
[Cleanup and safety](docs/11-cleanup-safety.md) ·
[Known locations](docs/12-known-locations.md) ·
[Cloud storage](docs/13-cloud-storage.md) ·
[Photo review](docs/14-photo-review.md) ·
[Companion](docs/15-companion.md) ·
[Optimise](docs/16-optimise.md)

**Engineering:**
[Architecture](docs/20-architecture.md) ·
[Tech stack](docs/21-tech-stack.md) ·
[Development](docs/22-development.md) ·
[Translations](docs/23-translations.md) ·
[Decisions](docs/24-decisions.md) ·
[Performance](docs/25-performance.md)

**Trust:**
[Privacy](docs/30-privacy.md) ·
[Glossary](docs/31-glossary.md) ·
[Threat model](docs/32-threat-model.md)

## Contributing

Contributions are welcome: code, known-location rules, translations and bug reports.
Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md). Every commit
carries a DCO sign-off.

## AI usage

Unhoard is built with the help of AI coding assistants, under human review. Every change,
whoever or whatever wrote it, has to pass the same tests, lints and safety checks, and
contributors disclose AI assistance in their pull requests. The rules are in
[AGENTS.md](AGENTS.md).

## License

Copyright (C) 2026 Kees van Wanrooij

This program is free software: you can redistribute it and/or modify it under the terms
of the GNU General Public License as published by the Free Software Foundation, either
version 3 of the License, or (at your option) any later version. It is distributed in
the hope that it will be useful, but WITHOUT ANY WARRANTY. See [LICENSE](LICENSE).

## Acknowledgements

Unhoard learns from the open-source disk tools that came before it, such as WinDirStat,
Czkawka and the many treemap projects, and from Microsoft's documentation of the Cloud
Files API. See [Market landscape](docs/03-market-landscape.md).
