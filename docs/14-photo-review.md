[Unhoard](../README.md) › [Docs](README.md) › Features › Photo review

# Photo review

Twenty years of phone photos hold a lot of near copies: bursts, WhatsApp re-sends,
screenshots and the same picture in two folders. Photo review groups them and lets you
clear a group in seconds, **without downloading a single online-only file**.
Acceptance criteria: F7 in [MVP scope](05-mvp-scope.md).

## What it works from

Only data that needs no download:

- file names, sizes and dates from the scan
- metadata from the Windows property system
- thumbnails from the Windows thumbnail API

Local files may also be hashed in full, to prove an exact duplicate. Online-only files
never are (S2).

## Recognising the source

The file name tells where most photos come from (T-7.1).

| Source | Pattern | Notes |
|---|---|---|
| WhatsApp | `IMG-YYYYMMDD-WA####.jpg`, `VID-YYYYMMDD-WA####.mp4` | EXIF stripped and recompressed |
| Samsung | `YYYYMMDD_HHMMSS.jpg` | |
| Google Pixel | `PXL_YYYYMMDD_HHMMSSmmm.jpg` | |
| iPhone | `IMG_####.HEIC` or `.JPG`, `.MOV` live photos, edited `IMG_E####` | |
| Windows Phone, Nokia Lumia | `WP_YYYYMMDD_###.jpg` | |
| Older Nokia | `Image###.jpg` style | (verify) |
| Screenshots | `Screenshot_*`, `Schermafbeelding *`, `Screenshot YYYY-MM-DD *` | |
| Telegram, Signal | their own patterns | (verify) |

## Groups

Each group says how sure the match is.

| Group | How it is found | Label |
|---|---|---|
| **Exact duplicates, local** | Same size, then a partial hash (first and last 64 KB), then a full BLAKE3 hash | "Identical" |
| **Exact duplicates, cloud-only** | Same size, same extension and an identical thumbnail perceptual hash | "Almost certainly identical", never "identical" |
| **Near duplicates** | Perceptual hash (dHash or pHash) Hamming distance at or below a threshold, tuned on generated images | "Very similar" |
| **Bursts** | Same source pattern, timestamps from the name or EXIF within about 2 seconds | "Burst of N" |
| **Chat media** | WhatsApp, Telegram and Signal patterns | "Chat media", grouped by month |
| **Screenshots** | Screenshot patterns | "Screenshots", grouped by month |

## Best shot

Within a burst or a group of near duplicates, Unhoard suggests one keeper, marked with a
star and its reasons:

1. **sharpness**: the variance of the Laplacian on the thumbnail
2. **resolution**
3. **file size**

Thumbnails are small, so the sharpness score can miss fine detail. The suggestion says so,
and it is a suggestion only.

## The review grid

- One group at a time, as a grid of thumbnails.
- **Nothing is selected when a group opens (S1).** "Select all but the suggested keeper"
  is one user action.
- Keyboard: arrows move, Space toggles, Enter confirms the group, N skips to the next
  group.
- Confirmed items go through the normal preview. Local items go to the quarantine; cloud
  items go to the cloud quarantine ([Cloud storage](13-cloud-storage.md#removing-cloud-items)).

## Thumbnails

- Thumbnails come from `IShellItemImageFactory::GetImage` with `SIIGBF_THUMBNAILONLY`, so
  Windows never renders a full image to make one. Calls run on a small pool of COM STA
  worker threads.
- For online-only files, OneDrive's thumbnail provider can serve the thumbnail without a
  download (verify in the manual checks). If a thumbnail cannot be obtained without
  hydration, the item is grouped by name, size and date only, and marked so.
- Unhoard decodes nothing itself beyond the thumbnail bitmap it receives, with fixed size
  limits ([Threat model](32-threat-model.md)).

## Videos

- Exact duplicates by size, duration and resolution, read from the property system:
  `System.Media.Duration`, `System.Video.FrameWidth` and `System.Video.FrameHeight`.
  Whether these are available on placeholders is part of the manual checks (verify).
- Similar videos (re-encoded or trimmed) need ffmpeg and are in Next.

## Local cache

Thumbnail hashes and content hashes are cached in `%LOCALAPPDATA%\Unhoard\cache\`, so a
second review is fast. The cache holds hashes and IDs, never images. It stays on the PC
and can be deleted in Settings ([Privacy](30-privacy.md)).

## Roadmap

- local AI photo categories and faces, on the device only
- similar videos with ffmpeg
- choosing the best copy among duplicates in different folders

See [Roadmap](06-roadmap.md).

---

## Related in this cluster

- [Cloud storage](13-cloud-storage.md): placeholders, the hydration guard and the cloud quarantine.
- [Cleanup and safety](11-cleanup-safety.md): where reviewed items go.
- [Companion](15-companion.md): the tips during a review.
- [Storage map](10-storage-map.md): where photo folders show up first.

Back to the [docs hub](README.md).
