[Unhoard](../README.md) › [Docs](README.md) › Product › Roadmap

# Roadmap

Three horizons, no version numbers and no dates. [Decisions](24-decisions.md) holds the
reason an item sits in its horizon.

- **Now:** what we are working on.
- **Next:** what comes right after the MVP, roughly in order of value.
- **Later:** ideas we believe in, but that need the basics to be solid first.

## Now

The MVP, described in full in [MVP scope](05-mvp-scope.md):

- [x] Research and intake decisions
- [ ] Docs (this set) and design explorations
- [ ] Second round of questions on what the docs and designs raised
- [ ] One-shot MVP build, test-first:
  - parallel scan without admin, with live build-up
  - the living cell map (default) and the treemap switch, synced with a list
  - the hidden-space cell with an elevated read-only "reveal" rescan
  - the known-locations library with explanations, risk tiers and actions
  - the central quarantine with manifest, restore and a 30-day purge
  - OneDrive awareness and "free up space"
  - photo and video review without hydration
  - the companion with scripted speech bubbles
  - English and Dutch
  - the CLI (`unhoard scan <path> --json`)
  - a per-user installer, a portable exe and a prepared winget manifest
- [ ] Public GitHub repository and the first release

## Next

| Item | Why it matters |
|---|---|
| **Optimise** (NL "Optimaliseren") | An honest check-up: startup apps, disk health and performance, free space and updates, Windows health, resource use and configuration. Measured findings in a list without a score, safe reversible actions, no speed promises. See [Optimise](16-optimise.md) |
| **MFT turbo-scan** (admin, NTFS) | Closes the speed gap with MFT-based tools for people who allow elevation |
| **Storage history and growth timeline** | Answers "what grew and why" by comparing saved scans over time |
| **Background insights with a tray icon** | On by default, light and local, and switchable in Settings. They power storage growth, startup impact and disk health, and the companion tells you when something changes |
| **AI companion chat with a local model** | Ask questions about your storage in plain language. Runs locally by default (a model of about 3 GB, downloaded only when you ask for it), sees only metadata through tools. See [Companion](15-companion.md#roadmap) |
| **OneDrive login through Microsoft Graph** | Quota and content hashes, so cloud duplicates become exact without downloads |
| **Synology and NAS** | Scan SMB shares and mapped drives; an archive assistant that moves cold files to the NAS |
| **Quarantine on another drive** | Moving to an external drive or NAS frees space at once instead of after 30 days |
| **Similar videos** (ffmpeg) | Find re-encoded or trimmed copies, not only exact duplicates |
| **Local AI photo categories and faces** | Sort photos by what is in them, on the device, never in the cloud |
| **Leftovers of uninstalled apps** | Find folders in AppData and ProgramData that belong to apps that are gone; Steam game leftovers are already recognised |
| **Opt-in update check** | Off by default; one request to GitHub Releases when switched on |
| **Other cloud providers tested** | Dropbox, iCloud for Windows, Box and Synology Drive use the same Cloud Files API; test and label them |
| **Explorer context menu** | "Show in Unhoard" on any folder |
| **Code signing and the Microsoft Store** | Fewer SmartScreen warnings and an easier install (SignPath Foundation for open-source signing) |
| **More languages** | Community translations once the English and Dutch texts are stable |

## Later

| Item | Why it matters |
|---|---|
| **macOS and Linux** | The core is portable by design; the platform layer needs new implementations |
| **Optional AI-credits subscription** | Only to cover cloud-AI costs for people who want it. Unhoard itself stays free and donation-funded; the local model stays free |
| **Bring your own API key** | Use a cloud model you already pay for, only after explicit consent |
| **Project recognition across locations** | See that a project lives in Documents, Downloads, a dev folder and OneDrive at once |
| **AI proposals for folder organisation** | Suggestions only, never automatic moves |
| **Smart storage tiering** | Keep hot files on the SSD, move cold ones to an HDD or NAS, with a manifest |
| **Choosing the best copy among duplicates** | Pick which location to keep (the organised year folder, not the dump folder) |
| **rclone for other services** | Google Drive, Dropbox and others beyond the Cloud Files API |

## Not planned

These keep coming up in this category. We do not plan to build them, because they
conflict with the [vision](01-vision.md):

- registry cleaning, "freeing" RAM, service tweaks, installing drivers, or any promise of
  "faster" (Optimise explains these myths instead)
- automatic cleanup without a user click
- telemetry or usage analytics of any kind
- hand-deleting WinSxS, System32, the Installer folder or ProgramData internals
- disabling the page file or deleting restore points blindly

---

## Related in this cluster

- [MVP scope](05-mvp-scope.md): the full detail of Now.
- [Vision](01-vision.md): what the roadmap must stay true to.
- [Competitive advantage](04-competitive-advantage.md): which gaps Next closes.
- [Market landscape](03-market-landscape.md): what others already offer.

Back to the [docs hub](README.md).
