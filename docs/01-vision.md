[Unhoard](../README.md) › [Docs](README.md) › Product › Vision

# Vision

> See where your space went. Understand what it is. Get it back safely.

## The problem

A typical story: a 1 TB SSD with 83 GB free. The owner has already deleted what they
could find. A fresh Windows install would take only a fraction of the disk, so where did
the rest go? The tools built into Windows show one folder at a time, they are slow, and
they never explain what a big folder *is*.

Existing disk analyzers answer half the question. They show **where** the space is, often
beautifully and very fast. They do not answer the next three questions every normal
person asks:

1. **What is this?** `ext4.vhdx`, `WinSxS`, `hiberfil.sys` and `Windows.old` mean nothing
   to most people.
2. **Is it safe to remove?** Guessing wrong breaks Windows, an app or a game, or loses
   photos.
3. **What should I do with it?** Some items are removed by Windows' own cleanup tool, some
   by uninstalling an app, some should simply stay.

Two things make it worse today:

- **Cloud placeholders.** With OneDrive Files On-Demand, a folder can hold hundreds of
  gigabytes "in the cloud" while using almost nothing locally, or the reverse. Most tools
  either count cloud files as zero or count them at full size, and none of them help you
  free the local copies.
- **Photo libraries.** Twenty years of phone photos, chat media, screenshots and bursts
  pile up. Disk tools do not understand them, and photo deduplicators do not understand
  the disk or the cloud.

## The promise

Unhoard is a free, open-source Windows app that:

1. **Shows where the space went.** A living cell map of your disk, built live while the
   scan runs, with a simple treemap one click away. Space the scan could not see is shown
   honestly as its own "hidden space" cell, with an explanation.
2. **Explains what each big item is.** A library of known locations gives every
   recognised item a plain-language explanation, a risk tier with a reason, and the right
   action.
3. **Helps you get it back safely.** Nothing is ever selected for you. What you remove
   goes to a central quarantine with a manifest, so you can restore it exactly where it
   was. After 30 days the quarantine is emptied.
4. **Understands the cloud.** It shows local size next to cloud size and can make
   OneDrive files online-only, without a login and without downloading anything.
5. **Helps sort a lifetime of photos.** Duplicates, bursts, chat media and screenshots
   are grouped for fast review. It never downloads cloud-only photos to do this.
6. **Optimises honestly** (in [Next](06-roadmap.md)). A check-up of startup apps, disk
   health, free space, Windows health and resource use. Every finding is measured, not
   guessed, and explained. The safe actions are reversible, and Unhoard makes no promise
   of "faster". See [Optimise](16-optimise.md).

A companion character guides you through all of it in speech bubbles instead of popups.

## Principles

These are the rules every feature is checked against.

1. **No deletion without context.** Every suggestion says what the item is, how risky it
   is to remove, and why.
2. **Every move is a user click.** Unhoard never selects, moves or deletes anything by
   itself. The one automatic action is emptying the quarantine after 30 days, and the
   companion warns you before that happens.
3. **Reversible by default.** Removal goes through the quarantine. Final deletion is a
   separate, later step.
4. **Honest about space.** A move on the same drive frees nothing until the quarantine is
   emptied, and Unhoard says so. Freed space is reported only when it is really free.
5. **Never touch the cloud by accident.** Online-only files are never downloaded, and
   cloud files are never moved into the local quarantine.
6. **Private by default.** No telemetry, ever. No network traffic unless you switch a
   feature on. See [Privacy](30-privacy.md).
7. **Calm shell, show on stage.** Navigation, lists and settings are calm and readable,
   like a good Windows 11 app. The centre stage (scan animation, cell map, companion) is
   where the motion and the futuristic command-center feel live.
8. **Explain, don't scare.** No "your PC is at risk", no inflated junk counters, no
   "boost". Unhoard is the opposite of the classic "PC cleaner".
9. **Light and switchable in the background.** Background insights (storage growth,
   startup impact, disk health) are on by default only because they give real insight.
   They stay local, use few resources, and can be switched off in Settings.

## What success looks like

A home user with a full disk opens Unhoard for the first time and, within about ten
minutes:

- understands the ten biggest consumers of their disk, including the hidden space
- knows for each one whether it is safe to remove and how
- frees a meaningful amount of space without breaking anything
- knows exactly where removed items went and how to get them back

A power user gets the same answers, plus a fast scan and a CLI with JSON output.

## Non-goals

Unhoard is not, and will not become:

- a registry cleaner, a driver updater, or a "booster" that promises speed
- an antivirus or privacy "scrubber"
- a backup or sync tool (it works *with* OneDrive, it does not replace it)
- a general file manager
- a tool that cleans anything in the background without asking

## The experience in three pillars

1. **Command center.** A premium, futuristic stage with motion: the scan is a show, the
   map is alive. Inspired by the feel of film-style holographic interfaces, built as our
   own design.
2. **Living map.** Organic cells (a Voronoi treemap) that grow while the scan runs and
   stand still once it is done, so the map is easy to read. The treemap stays available for people who prefer plain
   rectangles. See [Storage map](10-storage-map.md).
3. **A guide, not a popup.** An original companion character explains, suggests and
   warns in speech bubbles. See [Companion](15-companion.md).

---

## Related in this cluster

- [Target users](02-target-users.md): who this vision is for.
- [Market landscape](03-market-landscape.md): what already exists.
- [Competitive advantage](04-competitive-advantage.md): where this vision makes us
  different.
- [MVP scope](05-mvp-scope.md): the first build that delivers the promise.
- [Roadmap](06-roadmap.md): what comes after.

Back to the [docs hub](README.md).
