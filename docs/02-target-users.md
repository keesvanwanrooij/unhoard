[Unhoard](../README.md) › [Docs](README.md) › Product › Target users

# Target users

Unhoard is built first for home users with a full disk and a big photo library. Power
users and developers are the second group: they get speed, exact numbers and a CLI, but
the design is never bent so far toward them that a home user gets lost.

## Ideal user profile

| Trait | Description |
|---|---|
| Platform | Windows 10 or 11, a single PC or laptop, usually one SSD between 256 GB and 2 TB |
| Situation | The disk is (nearly) full and they do not know where the space went |
| Skill | Comfortable with Explorer and Settings; does not know what `WinSxS` or a `.vhdx` is |
| Cloud | Often OneDrive (personal or Microsoft 365 family), with Files On-Demand switched on |
| Photos | Many years of phone photos from several brands, a lot of chat media, often stored in OneDrive |
| Tried so far | Windows Storage settings, Disk Cleanup, maybe a "PC cleaner" they did not trust |
| Wants | A clear answer, a safe way to free space, and no risk of losing photos |
| Fears | Deleting something important, breaking Windows, cleaners that do too much |

## Personas

The personas are fictional composites. The numbers are examples.

### 1. Anna, the full-disk home user (primary)

- Laptop with a 1 TB SSD and 83 GB free. Windows keeps warning her.
- She already emptied Downloads and the Recycle Bin. Windows' storage page shows big
  "Other" and "System" bars that she cannot act on.
- **Job to be done:** "Show me what is using my disk and help me get 100 GB back without
  breaking anything."
- **What she needs from Unhoard:**
  - one overview of the whole disk, including the part a normal scan cannot see
  - a plain explanation of each big item
  - the safest action, done for her with one click, and a way back
- **She leaves if:** the app looks like a scary cleaner, asks for admin at once, or
  deletes something without asking.

### 2. Mark, the family photo keeper (primary)

- Twenty years of photos from four phone brands and years of chat apps, about 100 GB.
  Most of it lives in OneDrive, online-only, so it is not on the disk at all.
- Folders per year, plus several "dump" folders from old phones. Many duplicates, bursts
  and forwarded chat pictures.
- **Job to be done:** "Help me find the doubles, the bursts and the chat junk, so I keep
  the good photos and lose the clutter."
- **What he needs from Unhoard:**
  - groups he can review fast, with a suggestion of the best shot
  - no download of his whole cloud library, because his disk has no room for it
  - clear labels for how sure a match is
- **He leaves if:** the app starts downloading his OneDrive, or suggests deleting photos
  he cannot see first.

### 3. Sam, the power user and developer (secondary)

- Desktop with WSL, Docker, several game launchers, dozens of `node_modules` folders and
  package caches.
- Knows what most of it is, but has no time to hunt through `%LOCALAPPDATA%`.
- **Job to be done:** "Tell me exactly which caches, virtual disks and build folders are
  big, and let me script it."
- **What Sam needs from Unhoard:**
  - a fast scan
  - exact sizes (logical and on disk)
  - developer clutter grouped per project
  - a CLI with JSON output
  - the reveal of hidden space as admin
- **Sam leaves if:** the scan is slow, the numbers are vague, or the app hides details
  behind friendly text.

### 4. Lisa, the family helper (secondary)

- The person who "knows computers" and fixes the PCs of parents and friends.
- **Job to be done:** "Free space on someone else's PC safely, and leave them able to get
  things back if I removed something they needed."
- **What she needs from Unhoard:**
  - risk tiers she can trust
  - a quarantine the owner understands
  - a portable exe she can run without installing
  - Dutch for her parents and English for herself
- **She leaves if:** she has to explain every step, or a restore is not exact.

## What each group values most

| Need | Anna | Mark | Sam | Lisa |
|---|---|---|---|---|
| "Where did my space go" overview | ●●● | ●● | ●● | ●●● |
| Plain explanations and risk tiers | ●●● | ●● | ● | ●●● |
| Quarantine and exact restore | ●●● | ●●● | ●● | ●●● |
| OneDrive awareness, no hydration | ●● | ●●● | ● | ●● |
| Photo triage | ● | ●●● | – | ●● |
| Scan speed and exact numbers | ● | ● | ●●● | ●● |
| CLI and JSON | – | – | ●●● | ● |
| Motion and the companion | ●● | ●● | ● | ●● |

●●● = decisive, ●● = important, ● = nice to have, – = not relevant.

## Who Unhoard is not for

- **IT administrators managing fleets or servers.** Scheduled reports across machines,
  SQL export and NAS-wide analysis are what TreeSize Professional, SpaceObServer and
  DiskSavvy are for. See [Market landscape](03-market-landscape.md).
- **People who want a one-click "speed boost".** Unhoard's Optimise check-up shows
  measured facts and safe, reversible steps. It does not promise speed, clean the
  registry or tweak services.
- **People who want everything cleaned automatically.** Unhoard never selects for you.
  Windows' Storage Sense exists for fully automatic cleanup.

---

## Related in this cluster

- [Vision](01-vision.md): the problem these users share.
- [Market landscape](03-market-landscape.md): what these users can use today.
- [Competitive advantage](04-competitive-advantage.md): why they would pick Unhoard.
- [MVP scope](05-mvp-scope.md): which of their needs the first build covers.

Back to the [docs hub](README.md).
