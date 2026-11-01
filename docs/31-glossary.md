[Unhoard](../README.md) › [Docs](README.md) › Trust › Glossary

# Glossary

The terms used in the app and in these docs, with the Dutch term the app uses. Dutch
Windows terms follow Windows NL ([Translations](23-translations.md)).

## Sizes

| English | Dutch | Meaning |
|---|---|---|
| Size on disk (allocation size) | Grootte op schijf | The space a file really takes on the drive. Compressed files take less; online-only files take 0. Unhoard's default measure |
| Logical size | Grootte | The length of the file's content, as Explorer's "Size" column shows it |
| GB (binary) | GB | 1024³ bytes, the way Explorer counts. Drive makers use 10⁹ bytes, which is why a "1 TB" drive shows as about 931 GB |
| GiB | GiB | The technical name for 1024³ bytes; Unhoard shows "GB" like Explorer |

## Cloud

| English | Dutch | Meaning |
|---|---|---|
| Sync root | Synchronisatiemap | The local folder a cloud client keeps in sync, such as the OneDrive folder |
| Cloud Files API | Cloud Files API | The Windows interface cloud clients use for placeholders |
| Placeholder | Tijdelijke aanduiding | A file that exists in Explorer but whose content may live only in the cloud |
| Online-only | Alleen online | A placeholder without local content; it takes no space on disk |
| Locally available | Lokaal beschikbaar | The content is on the disk and may be freed again |
| Always keep on this device | Altijd behouden op dit apparaat | Pinned: the content stays on the disk |
| Hydration | Hydratatie (downloaden) | The cloud client downloading a placeholder's content. Unhoard never causes it |
| Free up space | Ruimte vrijmaken | Making files online-only again, without deleting them |
| Cloud quarantine | Cloudquarantaine | Unhoard's quarantine folder inside a sync root |

## Scanning

| English | Dutch | Meaning |
|---|---|---|
| Hidden space | Verborgen ruimte | Used space the scan could not see: (total − free) − scanned size |
| Reveal hidden space | Verborgen ruimte onthullen | A read-only rescan with admin rights that names part of the hidden space |
| MFT | MFT | The NTFS Master File Table; reading it directly is faster but needs admin (Next) |
| SeBackupPrivilege | SeBackupPrivilege | A Windows privilege that lets an elevated process read folders it has no access to |
| Hard link | Harde koppeling | One file reachable under several names; Unhoard counts it once |
| Junction, symbolic link | Koppelpunt, symbolische koppeling | A folder entry that points elsewhere; Unhoard records it and never follows it |
| Reparse point | Reparsepunt | The NTFS mechanism behind links, junctions and cloud placeholders |
| Unscanned | Niet gescand | A folder the scan could not read, listed with its reason |

## Cleanup

| English | Dutch | Meaning |
|---|---|---|
| Quarantine | Quarantaine | Unhoard's holding folder, one per drive, where moved items wait 30 days |
| Batch | Batch | All items moved in one confirmed action, with one purge date |
| Manifest | Manifest | The record of each moved item, written before the move, used to restore it |
| Restore | Terugzetten | Moving an item back to where it was, never overwriting |
| Purge | Definitief opruimen | The automatic permanent deletion of a batch after 30 days and a warning |
| Not freed yet | Nog niet vrijgemaakt | Space in the quarantine that returns only after the purge |
| Activity log | Activiteitenlogboek | The local list of every move, restore and purge |
| Known location | Bekende locatie | A library entry that explains a common big item |
| Safe | Veilig | Regenerable; the app or Windows rebuilds it |
| Likely safe | Waarschijnlijk veilig | Usually unneeded, but it was yours or an app's |
| Review | Bekijken | Only you can judge |
| Keep (system) | Behouden (systeem) | Needed; explain only |
| Protected path | Beschermd pad | A path Unhoard never moves or deletes |

## Visuals

| English | Dutch | Meaning |
|---|---|---|
| Cell map | Celkaart | Unhoard's default map of organic cells whose areas match the sizes |
| Power diagram | Machtsdiagram | The weighted Voronoi layout behind the cell map |
| Treemap | Treemap | The simple map of nested rectangles |
| Breadcrumb | Kruimelpad | The path above the map that shows where you are |
| Priority matrix | Prioriteitsmatrix | The view that plots suggestions by size, effort and last use |
| Quick win | Snelle winst | A big item that takes little effort to clear |
| Effort | Moeite | How much work clearing an item takes, in four levels |
| App adapter | App-koppeling | Code that reads an app's own records, such as Steam's game list |

## Photos

| English | Dutch | Meaning |
|---|---|---|
| Perceptual hash | Perceptuele hash | A short fingerprint of what an image looks like; similar images have similar hashes |
| Exact duplicate | Exact duplicaat | The same content, proven by a full hash for local files |
| Near duplicate | Bijna-duplicaat | A very similar image, such as a re-saved or slightly cropped copy |
| Burst | Reeks | Photos taken within about 2 seconds of each other |
| Best shot | Beste foto | The suggested keeper in a group |
| Chat media | Chatmedia | Photos and videos received through WhatsApp, Telegram or Signal |

## Companion

| English | Dutch | Meaning |
|---|---|---|
| Companion | Metgezel | Unhoard's guide character |
| Speech bubble | Tekstballon | How the companion speaks; never a popup |
| Quiet mode | Stille modus | Only warnings |

## Windows

| English | Dutch | Meaning |
|---|---|---|
| Storage Sense | Opslaginzicht (verify) | Windows' automatic cleanup of temp files, the Recycle Bin and cloud files |
| Disk Cleanup | Schijfopruiming | The classic Windows cleanup tool; "Clean up system files" includes Windows Update leftovers and previous installations |
| WinSxS | WinSxS | The Windows component store; Explorer overstates its size because of hard links |
| Windows.old | Windows.old | The previous Windows installation, kept for rollback |
| Page file | Wisselbestand | `pagefile.sys`, virtual memory; never disable it |
| Hibernation file | Sluimerstandbestand | `hiberfil.sys`, used for hibernate and Fast Startup |
| Restore point | Herstelpunt | A snapshot of system files for System Restore |
| Shadow copy | Schaduwkopie | The Volume Shadow Copy data behind restore points |
| UAC | Gebruikersaccountbeheer | The prompt that asks for admin rights |

## Project

| English | Dutch | Meaning |
|---|---|---|
| DCO | DCO | Developer Certificate of Origin: the `Signed-off-by` line that says you may contribute the code |
| Conventional Commits | Conventional Commits | The commit message format `type: summary` |
| Risk class | Risicoklasse | Low, Medium, High or Critical; sets how rigorous a change must be |

---

## Related in this cluster

- [Privacy](30-privacy.md): what stays on your PC.
- [Threat model](32-threat-model.md): the risks behind these terms.

Back to the [docs hub](README.md).
