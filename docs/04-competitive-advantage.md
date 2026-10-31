[Unhoard](../README.md) › [Docs](README.md) › Product › Competitive advantage

# Competitive advantage

Where Unhoard is better than what exists, where it is not, and what we do about the gaps.
The evidence is in [Market landscape](03-market-landscape.md). Everything here describes the
planned MVP; until it ships, these are goals, not facts.

## Positioning in one sentence

For Windows home users with a full disk and a big photo library, Unhoard is the free,
open-source storage command center that not only shows **where** the space went, but
explains **what** each item is, **how safe** it is to remove and **what to do**, and lets
you undo every step.

## Where we aim to be better

### 1. From "where" to "what, how safe, what to do"

- Analyzers stop at the map. Unhoard adds a [known-locations library](12-known-locations.md)
  that gives each recognised item a plain explanation, a risk tier with a reason, and the
  right action.
- The action can be done in Unhoard, through a Windows tool, or through an app's own
  uninstaller.
- The closest tools are DiskPilot (tiers with reasons, but no map and no quarantine) and
  Disk Cleanup (category descriptions, but fixed categories only).

### 2. A real quarantine, not just the Recycle Bin

- What you remove goes to one central quarantine with a manifest: original path, size and
  timestamps. Restore puts it back exactly where it was.
- After 30 days the quarantine is emptied, and the companion warns you before it happens.
- We are honest that a move on the same drive frees nothing until the quarantine is
  emptied.
- Nobody else combines a manifest, an exact restore and a timed purge. TreeSize Personal
  has undo for its operations, and SquirrelDisk can copy items away first.

### 3. Hidden space, explained

- Most tools show only what they scanned. Unhoard shows used space minus scanned space as
  its own cell, explains what usually hides there (restore points, other profiles,
  protected folders, file-system metadata) and offers an optional admin "reveal" rescan.

### 4. Cloud awareness that frees space

- For OneDrive, Unhoard shows local size next to cloud size for every folder, marks
  online-only, locally available and "always keep" files, and makes chosen items
  online-only with one click.
- It needs no login and never downloads anything.
- Windows' Storage Sense does this automatically by age. Explorer does it one item at a
  time. Neither shows you where the local copies are.

### 5. Photo-library triage, without downloads

- Duplicates, near duplicates, bursts, chat media and screenshots are grouped and
  reviewed in a fast grid: select all, uncheck the keepers.
- A best-shot suggestion helps pick the keeper.
- Online-only photos are analysed from names, metadata and shell thumbnails, never by
  downloading them.
- Disk tools do not do photo triage. Photo tools do not know about the disk or the cloud.

### 6. A guide instead of popups

- An original companion character explains, suggests and warns in speech bubbles. It
  turns a technical tool into something a home user can follow.

### 7. A show that stays usable

- A living cell map and a futuristic command center make the tool enjoyable to open.
  The shell around the stage stays calm and readable, and a simple treemap is one click
  away.
- SquirrelDisk is the closest in visual ambition. Diskplot deliberately chose a
  one-colour map.

### 8. Trust by design

- GPL-3.0-or-later, no telemetry ever, and zero network traffic in the MVP. Nothing is
  selected for you.
- In a category known for scareware, these are reasons to choose us.

### 9. English and Dutch from day one

- Dutch-speaking home users are underserved by English-only tools. A real Dutch interface,
  written in Windows' own Dutch terms, is a small but real edge in that market.

## Where we are not better (and will not pretend to be)

| Area | Who is better | Why | Our answer |
|---|---|---|---|
| Raw scan speed | WizTree, WinDirStat as admin, ClutterCutter | They read the NTFS MFT directly. Our MVP scans without admin through the normal directory APIs | Be fast enough without admin (target in [MVP scope](05-mvp-scope.md)), show results live, add an MFT turbo-scan in [Next](06-roadmap.md) |
| Polish and maturity | WinDirStat, SquirrelDisk | Years of releases, many users, many bug reports fixed | Keep the MVP scope tight and well tested; earn trust release by release |
| Number of languages | SquirrelDisk (25) | Community translations over time | Start with English and Dutch, make adding a language easy ([Translations](23-translations.md)) |
| Platforms | SquirrelDisk, Czkawka (cross-platform) | We start on Windows only | Keep the core portable; macOS and Linux are in [Later](06-roadmap.md) |
| Exact cloud duplicates | None today, but Microsoft Graph hashes would be exact | Without a login we cannot get OneDrive's content hashes | Label cloud matches "almost certainly identical"; add Graph login in Next |
| Photo similarity quality | Commercial photo tools with on-device image models | We use perceptual hashes of thumbnails, which miss crops and edits | Be honest about match confidence; local AI categories are in Next |
| Features for administrators | TreeSize Professional, DiskSavvy | Scheduling, SQL, servers and NAS reporting | Not our audience ([Target users](02-target-users.md)) |
| Install trust on day one | Store and signed apps | Our early builds are unsigned, so SmartScreen warns | Explain it in the README FAQ, publish checksums, pursue free code signing (SignPath Foundation) in Next |
| Track record | Everyone | We are new | Open development, honest docs, and a safety design that is easy to audit |

## Threats

- **Microsoft improves the built-in tools.** Storage settings and PC Manager could add
  explanations or a map. Our answer: depth (the known-locations library, the quarantine,
  photo triage) and independence. We link to Windows tools rather than compete with
  them.
- **Storage Sense is switched on.** It already makes unused OneDrive files online-only.
  Our answer: we show what it did and what remains, and let people choose per item.
- **"Cleaner" distrust.** Users may lump us in with scareware. Our answer: open source,
  no telemetry, no scary numbers, nothing selected for you, every move reversible, and
  official downloads only from GitHub Releases and winget.
- **Impersonation.** Popular open-source tools get copied onto malware download sites.
  Our answer: an "official downloads" section in the README and checksums for every
  release.

## What is hard to copy

- The **known-locations library**: curated, explained, risk-rated and maintained by the
  community, in two languages. It grows in value with every contribution.
- The **safety model** as a whole: no auto-selection, quarantine with manifest, no
  hydration, protected paths and honest space accounting. Each part is simple; together
  they build trust that a feature list cannot.

## Messaging rules

| Do | Don't |
|---|---|
| "See where your space went." | "Your PC is at risk!" |
| "This is safe to remove because it rebuilds itself." | "Found 12,483 junk files!" |
| "Moved to quarantine. Space is freed when you empty it." | "Freed 40 GB!" (when nothing is freed yet) |
| "We never download your cloud photos." | "Optimise your cloud." |
| Compare honestly and link to other tools | Claim to be the fastest or the best |
| "3 of your 14 startup apps have high impact, as measured by Windows." | "Speed up your PC by 300%!" |
| "Freeing RAM does not make Windows faster, because Windows uses free memory as cache." | "Free 4 GB of RAM now!" |

---

## Related in this cluster

- [Market landscape](03-market-landscape.md): the tools we compare against.
- [Vision](01-vision.md): the principles behind these advantages.
- [MVP scope](05-mvp-scope.md): what makes these advantages real in the first build.
- [Roadmap](06-roadmap.md): how we close the gaps listed above.

Back to the [docs hub](README.md).
