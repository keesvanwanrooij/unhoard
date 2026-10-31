[Unhoard](../README.md) › [Docs](README.md) › Product › Market landscape

# Market landscape

What people can use today to understand and free disk space on Windows, and what each
tool does well. We checked this on **2026-10-31** from the projects' own READMEs and
websites, the GitHub API and a few independent reviews (sources at the bottom). We did
not run hands-on benchmarks.

**How to read the claims**

- Only what we saw in a primary source (the project's own README, docs or website) is
  stated as fact.
- **(verify)** marks a claim we could not confirm from a primary source.
- When we found nothing about a feature, the matrix says "–". That means "not found in
  our check", not "proven absent".

## The categories

1. **Disk analyzers** show where the space is: treemaps, sunbursts, sorted lists.
2. **Cleaners** remove known clutter (temp files, caches, update leftovers).
3. **Built-in Windows tools**: Storage settings, Storage Sense, Disk Cleanup, OneDrive's
   own context-menu actions, Microsoft PC Manager.
4. **Duplicate and similar-photo finders** compare files by hash or by image similarity.

Nobody combines all four with cloud awareness and a reversible removal flow. That gap is
covered in [Competitive advantage](04-competitive-advantage.md).

## Open-source disk analyzers

| Project | Stack | License | Stars* | Strengths |
|---|---|---|---|---|
| [WinDirStat](https://github.com/windirstat/windirstat) | C++ (MFC) | GPL-3.0 | ~4,400 | The classic. Treemap, sunburst and flame graph. MFT acceleration when run as admin, multiple drives, MTP devices. Duplicate search with configurable hashes and safeguards for cloud files; optional sparse hashing labelled "probable duplicates". CSV/JSON save and load, CLI (`/SaveTo`), cleanup commands, hardlink dedup. Distributed through the Microsoft Store, winget, Chocolatey and Scoop |
| [SquirrelDisk](https://github.com/adileo/squirreldisk) | Rust + egui | AGPL-3.0 | ~2,100 | Sunburst and treemap with a live build-up during the scan. A "collector tray" that sends items to the Recycle Bin, deletes them, or copies them to an external drive or the cloud first; optional secure erase; protected system paths. Remote storage (SSH, S3, Google Drive, Dropbox and more) through rclone. Explorer context menu, `scan --json` CLI, 25 languages, small app size. Has an AI-usage disclaimer and an AI pull-request policy |
| [NeoTreescan](https://github.com/neortal-technologies/NeoTreescan) | C# .NET WPF | Apache-2.0 | ~1 | Read-only analyzer. Parallel `FindFirstFileExW` with large fetch, `SeBackupPrivilege` in admin mode to see protected folders, long and UNC paths. Breakdowns by type, age and size, top 1000 files, Excel export. No telemetry |
| [Diskplot](https://github.com/rodrigokiller/diskplot) | Electron + TypeScript + React | MIT | 0 | Treemap and tree side by side, synced, with a live mode. Snapshots that show what grew, what is new and what is gone. Totals developer clutter. Instant search. Duplicates by size, then hash. A one-colour map on purpose. A compact typed-array data layout |
| [ClutterCutter](https://github.com/StruisICT/ClutterCutter) | Rust, Win32 + egui | Inconsistent (MIT file, README says "all rights reserved") | ~1 | Raw parallel MFT parsing as admin, about 2 million files in about 5 seconds by the author's own figure, with a `FindFirstFileEx` fallback. Scans all drives. Side panel with top files, oldest files and safe temp files, recycled in place. winget and MSI |
| [DiskPilot](https://github.com/tehgee42/DiskPilot) | Python + CustomTkinter | MIT | small | Risk tiers (SAFE / RECOMMENDED / SUGGESTED / ASK) with a reason per file. Finds old installers, duplicates (keeps the newest), developer folders. Removal goes to the Recycle Bin; there is no quarantine. Very early (a handful of commits) |
| [Czkawka / Krokiet](https://github.com/qarmin/czkawka) | Rust (Slint GUI for Krokiet) | MIT (repo), GPL-3.0 (Krokiet) | large (verify count) | Not a disk map, but a strong "find clutter" toolkit: duplicates, similar images and videos, music, empty files and folders, big files, temp files, broken files, bad extensions. `czkawka_core` is a reusable crate. Cross-platform |

\* Star counts from the GitHub API on 2026-10-31, rounded.

## Commercial and freeware analyzers

| Product | Model | Strengths | Notes |
|---|---|---|---|
| [WizTree](https://diskanalyzer.com/) | Free for personal use; a supporter code covers commercial use; Enterprise and Distribution licenses exist | Reads the NTFS MFT directly, which makes it the speed reference. Treemap, duplicates, CSV export and a CLI | Windows only; we found no macOS version |
| [TreeSize](https://www.jam-software.com/treesize/editions.shtml) (JAM Software) | Free (non-commercial), Personal about €20/user/year, Professional from about €41/user/year, Consultant and Enterprise (SpaceObServer) | Free: treemap, folder sizes, PDF export. Personal: checksum duplicates, snapshot compare, scan history, exports, hardlink dedup, file operations with undo, some cloud and network targets on a home network. Professional adds scheduling, CLI and portable use | OneDrive and Dropbox are not listed for any edition. Prices from the editions page on 2026-10-31 |
| [FolderSizes](https://www.foldersizes.com/) | Free edition plus paid editions | Treemap, sunburst, bar and pie charts, many report types, search, duplicates. Paid: export, scheduled reports, trend analysis, snapshots, CLI | Exact prices not verified (verify) |
| [DiskSavvy](https://www.disksavvy.com/) (Flexense) | Paid, with Server and Enterprise editions | Classification plugins, pie and history charts, SQL integration, CLI | Aimed at administrators and NAS storage |
| SpaceSniffer | Freeware | Treemap with zoom | Older project; current status not checked (verify) |
| DaisyDisk, GrandPerspective | Paid / free, macOS only | Sunburst (DaisyDisk) and treemap (GrandPerspective) | Not on Windows; design references only |

## Cleaners

| Product | What it does | Notes |
|---|---|---|
| [Microsoft PC Manager](https://www.windowslatest.com/2024/05/11/microsoft-pc-manager-performance-booster-for-windows-11-gets-files-cleanup/amp/) | Free, by Microsoft. Deep cleanup per app cache. "Files Cleanup" for downloads, large files and duplicates, with a "smart select oldest" option | Independent reviews report cleanup estimates that did not match the result, and a risk of selecting both copies of a duplicate pair ([Adam Fowler](https://adamfowlerit.com/2024/05/hands-on-with-microsofts-own-pc-cleaner-and-utility-toolkit-microsoft-pc-manager/)) |
| CCleaner | Long-running commercial cleaner | Has faced well-publicised privacy and security controversies. It shows why trust is the main currency in this category |

## Built into Windows and OneDrive

These are free, already installed, and often enough for simple cases. We link to them
from Unhoard instead of copying them.

| Tool | What it does well | What it lacks |
|---|---|---|
| **Settings › System › Storage** | Category bars (apps, temporary files, system, other) and cleanup suggestions. Windows 11 also lists large or unused files and files synced to the cloud (verify the exact list per Windows build) | No map, no drill-down across the whole disk, little explanation of "System" and "Other" |
| **Storage Sense** | Automatic cleanup of temporary files, the Recycle Bin and Downloads after a period you choose. It can also make locally available OneDrive files **online-only** when they have not been opened for a while; Microsoft documents 30 days as the default since Windows 11 22H2. Files marked "Always keep on this device" are exempt | Fully automatic, rule-based, no per-item insight. Microsoft documents Storage Sense as off by default; some sources say new installs enable it (verify per PC) |
| **Disk Cleanup** (`cleanmgr`) | Describes each category it cleans. "Clean up system files" handles Windows Update leftovers, Delivery Optimization files and "Previous Windows installation(s)" | Old-style UI, only fixed categories, no overview |
| **OneDrive in Explorer** | Per file or folder: "Free up space" (online-only) and "Always keep on this device". Microsoft documents the same states for `attrib`: `+u` online-only, `-p` locally available, `+p` always available | One item at a time; it does not show where the local copies are or how big they are |

## Duplicate and similar-photo finders

- Commercial tools include Cisdem Duplicate Finder, Duplicate Cleaner Pro, MindGems
  Duplicate Photo Finder, Ashisoft Duplicate File Finder and Gemini (macOS).
- Cisdem says it uses on-device image-similarity models; we have only the vendor's
  claim.
- Most online comparisons are affiliate listicles, so we did not rely on their rankings.
- None of the tools we looked at understands the disk as a whole, cloud placeholders, or
  phone and chat file-name patterns. They compare files they can read, which for
  online-only cloud photos means downloading them first.

## Feature matrix

Legend: ✓ yes · ~ partly · – not found in our check · ? unverified. "Unhoard" is the
planned MVP, not a shipped product.

| Tool | Visual map | Live build-up | Fast scan (MFT) | Explains items + risk | Reversible removal | Cloud-aware sizes | Free up cloud space | Photo triage | Duplicates | CLI / export | Price |
|---|---|---|---|---|---|---|---|---|---|---|---|
| WinDirStat | ✓ treemap, sunburst, flame | – | ✓ (admin) | – | ~ Recycle Bin (?) | ~ duplicate safeguards | – | – | ✓ | ✓ CLI, CSV/JSON | Free, GPL |
| SquirrelDisk | ✓ sunburst, treemap | ✓ | – | – | ~ tray to Recycle Bin or copy first | ~ cloud-only counts as 0 | – | – | – | ✓ JSON | Free, AGPL |
| WizTree | ✓ treemap | – | ✓ | – | ? | – | – | – | ✓ | ✓ CLI, CSV | Free personal |
| TreeSize Free | ✓ treemap | – | ? | – | – | – | – | – | – | ~ PDF | Free personal |
| TreeSize Personal | ✓ | – | ? | – | ~ undo for operations | – | – | – | ✓ checksum | ✓ | ~€20/yr |
| FolderSizes | ✓ several charts | – | ? | – | – | – | – | – | ✓ | ✓ (paid) | Free + paid |
| DiskSavvy | ✓ charts | – | ? | ~ classification | – | – | – | – | ? | ✓ CLI, SQL | Paid |
| NeoTreescan | ✓ treemap | – | – (stub) | – | read-only | – | – | – | – | ✓ Excel | Free, Apache |
| Diskplot | ✓ treemap | ✓ | – | ~ developer clutter | – | – | – | – | ✓ | – | Free, MIT |
| ClutterCutter | – | – | ✓ (admin) | ~ safe temp files | ~ Recycle Bin | – | – | – | – | – | Unclear |
| DiskPilot | – | – | – | ✓ tiers + reason | ~ Recycle Bin | – | – | – | ✓ | – | Free, MIT |
| Czkawka / Krokiet | – | – | – | – | ~ | – | – | ~ similar images | ✓ | ✓ CLI | Free, MIT/GPL |
| Windows Storage + Storage Sense | ~ bars | – | n/a | ~ categories | – | ~ | ✓ automatic, rule-based | – | – | – | Built in |
| Disk Cleanup | – | – | n/a | ~ per category | – | – | – | – | – | – | Built in |
| PC Manager | – | – | ? | ~ | ? | – | – | – | ✓ (see review notes) | – | Free |
| Photo duplicate finders | – | – | – | – | varies | – (need local files) | – | ~ similarity | ✓ | varies | Mostly paid |
| **Unhoard (planned MVP)** | ✓ living cells + treemap | ✓ | – (Next) | ✓ known locations, tiers + reason | ✓ quarantine, manifest, restore, 30-day purge | ✓ local vs cloud size | ✓ per item, by your click | ✓ duplicates, bursts, chat media, screenshots | ✓ | ✓ `scan --json` | Free, GPL |

## What the landscape tells us

1. **Speed is solved.** WizTree, WinDirStat (as admin) and ClutterCutter read the MFT.
   We should not try to win on raw speed in the MVP; we should be fast enough without
   admin and add an MFT scan later.
2. **Polish is high.** WinDirStat and SquirrelDisk are mature, well maintained and
   translated into many languages.
3. **Explanation is rare.** Only DiskPilot (tiers with reasons) and Disk Cleanup (category
   descriptions) explain anything, and neither covers the whole disk.
4. **Reversibility is weak.** The best on offer is the Recycle Bin, TreeSize Personal's
   undo, or SquirrelDisk's "copy first". Nobody has a quarantine with a manifest and a
   timed purge.
5. **Cloud is shallow.** Tools either count cloud-only files as zero or protect them from
   duplicate hashing. Windows' Storage Sense frees cloud space automatically, but by
   rule, not by insight.
6. **Photo triage lives in a different category.** Photo deduplicators do not know the
   disk or the cloud, and disk tools do not know photos.
7. **Trust is the currency.** The cleaner category has a history of scareware and privacy
   problems. Open source, no telemetry and honest numbers are a real differentiator, not
   decoration.

## Sources

- Repositories: GitHub API and READMEs, 2026-10-31.
- [WizTree](https://diskanalyzer.com/) · [TreeSize editions](https://www.jam-software.com/treesize/editions.shtml)
  · [FolderSizes](https://www.foldersizes.com/) · [FolderSizes vs TreeSize](https://www.foldersizes.com/features/treesize)
  · [DiskSavvy](https://www.disksavvy.com/)
- PC Manager reviews: [Windows Latest](https://www.windowslatest.com/2024/05/11/microsoft-pc-manager-performance-booster-for-windows-11-gets-files-cleanup/amp/)
  · [Adam Fowler](https://adamfowlerit.com/2024/05/hands-on-with-microsofts-own-pc-cleaner-and-utility-toolkit-microsoft-pc-manager/)
- Microsoft: [Query and set Files On-Demand states](https://learn.microsoft.com/en-us/onedrive/files-on-demand-windows)
  · [Manage drive space with Storage Sense](https://support.microsoft.com/windows/654f6ada-7bfc-45e5-966b-e24aded96ad5)
  · [Restore deleted files or folders in OneDrive](https://support.microsoft.com/office/949ada80-0026-4db3-a953-c99083e6a84f)
  · [Cloud Files API](https://learn.microsoft.com/en-us/windows/win32/cfapi/cloud-files-api-portal)

---

## Related in this cluster

- [Competitive advantage](04-competitive-advantage.md): where we are better and where we
  are not, based on this landscape.
- [Target users](02-target-users.md): who chooses between these tools.
- [Vision](01-vision.md): the gap we aim at.
- [Roadmap](06-roadmap.md): when we close our own gaps, such as the MFT scan.

Back to the [docs hub](README.md).
