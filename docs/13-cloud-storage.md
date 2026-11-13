[Unhoard](../README.md) › [Docs](README.md) › Features › Cloud storage

# Cloud storage

Unhoard understands OneDrive placeholders. It shows how much of a cloud library sits on
the disk and how much lives only in the cloud, and it frees local space without ever
downloading a file. The MVP reads the local sync folder and needs no sign-in.
Acceptance criteria: F6 in [MVP scope](05-mvp-scope.md).

## Finding sync roots

- Sync roots are read from the registry, where Windows records each provider's roots per
  user: `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\*\UserSyncRoots`,
  value named after the user's SID.
- The OneDrive personal folder is added from
  `HKCU\Software\Microsoft\OneDrive\Accounts\Personal\UserFolder`.
- **OneDrive personal is tested.** Other providers that use the Cloud Files API (Dropbox,
  iCloud for Windows, Box, Synology Drive) show as "detected, not tested".
- Google Drive for desktop is a virtual drive, not a Cloud Files API sync root. Unhoard
  skips it by default and says why.

## Placeholder states

Windows marks each file in a sync root with attributes. Unhoard reads them from the
directory listing, without opening the file.

| State (Explorer wording) | Attribute | Takes space on disk |
|---|---|---|
| Online-only | `RECALL_ON_DATA_ACCESS` (0x400000) | No (0 bytes) |
| Locally available | none of the cloud pin attributes | Yes |
| Always keep on this device | `PINNED` (0x80000) | Yes |
| Free up space requested | `UNPINNED` (0x100000) | Until the provider frees it |

For every folder in a sync root, the map and the Cloud view show:

- the **cloud size** (logical size of all files)
- the **local size** (size on disk)
- the counts of online-only, locally available and always-keep files

## Free up space

"Free up space" makes files online-only. It is the same as Explorer's "Free up space" and
as `attrib -p +u`.

1. **Preview:** the file count, the local size to be freed, and skipped files with a
   reason.
2. **Checks per file:** the file must be in sync (`CfGetPlaceholderInfo`). Files that are
   not uploaded yet are skipped, because freeing them would lose the only copy (AC6.4).
3. **Action:** only the pin state changes: clear `PINNED` and set `UNPINNED`, through
   `CfSetPinState` on a handle opened for attributes only, or through
   `SetFileAttributesW`.
4. **Re-measure:** after the provider frees the data, Unhoard re-reads the allocation size
   and reports what was really freed (S10).

## The hydration guard

**Online-only files are never hydrated (S2).** Many libraries are larger than the free
space on the disk, so one accidental download of a photo library could fill the drive.

- No code path opens a file inside a sync root with `FILE_READ_DATA`. Listings, attributes
  and pin states never need it.
- The fake platform layer records every open, and a test fails on any data open inside a
  sync root (T-6.2).
- In real runs, Unhoard re-checks the allocation size of a sample of files after thumbnail
  calls. If a file gained local data, the photo pipeline stops and the companion explains
  what happened.
- Listing a folder that was never opened may make the provider fetch its file list. That
  is metadata work by OneDrive, not a download of content.

## Removing cloud items

Cloud items never enter the local quarantine (S3): moving them out of the sync root would
download them and delete them from the cloud. Instead, each sync root gets its own
**cloud quarantine**: a folder `Unhoard Quarantine` at the top of the sync root.

- **Move in:** a rename inside the same sync root, through a handle opened without data
  access. OneDrive syncs it as a server-side move, so nothing is downloaded and the files
  stay restorable from any device. It follows the same steps as a local move: preview,
  identity check, manifest written first, per-item report
  ([Cleanup and safety](11-cleanup-safety.md#the-quarantine)). Whether the rename leaves
  online-only files online-only is part of the manual checks
  ([Development](22-development.md#manual-checks)) (verify).
- **Manifest:** the batch manifest sits next to the items in the cloud quarantine, so it
  syncs with them. Unhoard reads a manifest only while it is on this PC; if OneDrive
  made it online-only, that batch is not shown until it is local again, because
  reading it would download it.
- **Purge:** after 30 days, and after the companion warning, the purge deletes the batch
  inside the sync root. OneDrive moves it to its online recycle bin, where it stays
  another 30 days for personal accounts. A cloud item is restorable for about 60 days in
  total.
- **Restore:** moves the items back to their original folders inside the sync root, with
  the same no-overwrite rule as a local restore.

## Storage Sense

Storage Sense can also make OneDrive files online-only after they have not been opened
for a number of days (30 by default since Windows 11 22H2). Files on "always keep" are
exempt. Unhoard shows whether Storage Sense is on and links to its settings, so the user
can choose either tool, or both.

## Roadmap

- **OneDrive sign-in through Microsoft Graph:** quota, and `quickXorHash` content hashes,
  so cloud duplicates become exact without downloads. Needs an Entra app registration
  (public client).
- **Synology and NAS:** scan SMB shares and mapped drives, plus an archive assistant.
- **rclone** for providers outside the Cloud Files API.

See [Roadmap](06-roadmap.md).

---

## Related in this cluster

- [Photo review](14-photo-review.md): reviewing a cloud photo library without downloads.
- [Cleanup and safety](11-cleanup-safety.md): the local quarantine and the purge.
- [Storage map](10-storage-map.md): the cloud view and ghost outlines.
- [Known locations](12-known-locations.md): the explanations for local items next to the cloud ones.

Back to the [docs hub](README.md).
