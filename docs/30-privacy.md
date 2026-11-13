[Unhoard](../README.md) › [Docs](README.md) › Trust › Privacy

# Privacy

A disk tool sees your whole PC: every file name, every folder, your photos. Unhoard keeps
all of it on your PC.

## The promise

- **No telemetry, ever.** No usage analytics, no crash uploads, no "anonymous
  statistics".
- **No network in the MVP.** The app opens no network connection at all (S8). A CI check
  proves that no HTTP client is in the build (T-S8).
- **Network features are opt-in and come later.** The update check (Next) is off until you
  switch it on and then makes one request to GitHub Releases. A OneDrive sign-in (Next)
  happens only when you start it.
- **Nothing is read beyond metadata** unless a feature needs it and says so: local exact
  duplicates are hashed, and nothing else reads file contents.
- **Online-only cloud files are never downloaded** (S2).

## What stays on your PC

| What | Where | What it holds | How to remove it |
|---|---|---|---|
| Settings | `%APPDATA%\Unhoard\settings.json` | Language, view, motion, measure, companion mode | Delete the file |
| Scan results | Memory only | The scanned tree | Closing the app; nothing is saved unless you export (S9) |
| Batch manifests | Inside each quarantine folder | Original paths, sizes, timestamps, file IDs | Deleted with the batch by the purge |
| Activity log | `%LOCALAPPDATA%\Unhoard\activity.jsonl` | Time, operation, item IDs, result; never paths or file contents | Delete the file |
| Background insights (Next) | `%LOCALAPPDATA%\Unhoard\insights\` | Daily sizes of the largest folders, startup impact, disk health; at most 6 MB, 2 years | Settings › Optimise › turn off and clear |

Uninstalling Unhoard removes the program. Your settings and caches stay until you delete
them, and quarantine folders always stay, so an uninstall never deletes your files.

## Exports and the CLI

- An export happens only when you ask for one.
- Exports and CLI output replace your profile path with `%USERPROFILE%`, so a shared
  report does not show your user name (S9). A test proves the redaction works.
- Unhoard writes no log besides the activity log, which holds no paths.

## Background insights

Background insights arrive with Optimise in Next. They are **on by default**, because
growth over time is only visible if it is measured from the start.

- The first run says that they are on and how to turn them off.
- They measure folder sizes, startup impact and disk health. They never read file
  contents and store no file names below the largest folders.
- They stay on your PC and never use the network.
- Limits: 0.3% of one CPU core averaged over a day, 6 MB of storage, 2 years of
  retention ([Optimise](16-optimise.md#limits)).

## Elevation

- The main app never runs as administrator (S7).
- "Reveal hidden space" and "Read protected folders" start a separate helper after a
  UAC prompt. The helper only reads folder listings, sends the results back to the app,
  and exits. It cannot move, change or delete anything.

## What Unhoard reads besides your files

All of it stays on your PC:

- Windows' list of installed programs, to name install folders.
- Windows' program history (UserAssist), only the last start time of programs, to show
  when you last used them.
- Steam's and Ollama's own records of what is installed.
- After you click, the output of `docker system df`, which Docker answers locally.

## AI, later

- The AI companion (Next) runs a **local model** by default, downloaded only when you
  ask for it.
- The model sees only metadata from the scan index, through tools. It reads no file
  contents unless you explicitly allow it.
- A cloud model or your own API key is used only after explicit consent, and the app shows
  what is sent.

## Reporting a privacy issue

If Unhoard sends, stores or shows something this page says it does not, report it
privately as described in [SECURITY.md](../SECURITY.md).

---

## Related in this cluster

- [Threat model](32-threat-model.md): how these promises are protected.
- [Glossary](31-glossary.md): the terms used on this page.

Back to the [docs hub](README.md).
