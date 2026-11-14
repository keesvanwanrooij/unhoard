[Unhoard](../README.md) › [Docs](README.md) › Trust › Privacy

# Privacy

A disk tool sees your whole PC: every file name, every folder, your photos. Unhoard keeps
all of it on your PC.

## The promise

- **No telemetry, ever.** No usage analytics, no crash uploads, no "anonymous
  statistics".
- **Nothing leaves your PC.** Unhoard opens no connection to another computer (S8). A CI
  check proves that no HTTP client is in the build (T-S8). The one connection it can make
  is to Ollama on this PC, for the AI chat, only while you have the chat switched on;
  Ollama runs the model locally. The window's browser engine (Edge WebView2) starts with
  its component downloads and background networking switched off, so it fetches no DRM
  modules, filter lists or speech data either.
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
| Storage history and background insights | `%LOCALAPPDATA%\Unhoard\insights\history.jsonl` | Per snapshot: sizes and file counts of up to 1,000 of the largest folders, drive totals; at most 20 MB, 2 years | Settings › Background insights › turn off and clear |
| The daily measurement | A scheduled task for your account in Task Scheduler | When to run `unhoard-app.exe --measure` | Turning background insights off, or uninstalling |
| AI chat | Memory only | The conversation | Closing the app |
| The window's browser profile | `%LOCALAPPDATA%\io.github.keesvanwanrooij.unhoard\EBWebView` | Edge WebView2's own state and shader caches, about 15 MB; no HTTP cache and no downloaded components, which Unhoard removes at start | Delete the folder while Unhoard is closed |

Uninstalling Unhoard removes the program. Your settings and caches stay until you delete
them, and quarantine folders always stay, so an uninstall never deletes your files.

## AI chat

- Off until you switch it on in Settings.
- It talks only to Ollama on this PC (`127.0.0.1:11434`); a remote address is refused in
  code. The model runs on your PC, so your question and the tool results stay on it.
- The model sees file and folder names, sizes and explanations from the scan through
  tools, never file contents.
- The conversation is kept in memory only.
- Two things reach the internet, each only on a click in Settings: the Ollama download page
  opens in your browser, and Ollama itself downloads a recommended model from its library.
  Unhoard sends nothing in either case.

## Exports and the CLI

- An export happens only when you ask for one.
- Exports and CLI output replace your profile path with `%USERPROFILE%`, so a shared
  report does not show your user name (S9). A test proves the redaction works.
- Unhoard writes no log besides the activity log, which holds no paths.

## Background insights

Background insights power [Storage history](17-storage-history.md) and Optimise. They are
**on by default**, because growth over time is only visible if it is measured from the
start.

- The first run says that they are on and how to turn them off.
- They measure folder sizes, startup impact and disk health. They never read file
  contents and store no file names below the largest folders.
- They stay on your PC and never use the network.
- A scheduled task for your account runs the daily measurement while the app is closed;
  turning background insights off removes it.
- Limits: 0.29% of one CPU core averaged over a day, 20 MB of storage, 2 years of
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
