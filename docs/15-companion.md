[Unhoard](../README.md) › [Docs](README.md) › Features › Companion

# Companion

The companion is Unhoard's guide: a small original character that explains what you see
and what to do next, in speech bubbles instead of popups. In the MVP it speaks from a
script and uses no AI. Acceptance criteria: F8 in [MVP scope](05-mvp-scope.md).

## The character

The companion is **Lumen**, an original character: a small holographic orb that fits the
command-center look of the app ([Storage map](10-storage-map.md#look-and-feel)).

- **Shape:** a glowing core with a soft halo and two thin orbit rings, one dashed, that
  turn slowly in opposite directions.
- **Face:** two rounded eyes on the core, which blink now and then. No mouth, no limbs.
- **Colour:** the accent colours of the current theme (cyan and ice in the dark theme).
- **Motion:** it floats gently. While it speaks, it floats and pulses a little faster.
  With reduced motion or the "calm" setting it stands still.
- **Size and place:** about 96 px, in the bottom corner of the stage. Drag it to put it
  anywhere on the stage; Unhoard remembers the spot, and the bubble opens towards the
  middle of the stage so it always fits.
- The name works in English and Dutch.

### What we avoid

The companion is a modern take on the idea of a desktop assistant, and deliberately not
a copy of any existing one:

- no paperclip, and no office object with eyes
- no unsolicited interruptions: it never pops up over your work or steals focus
- no "It looks like you're…" guessing; it reacts to what you did or what the app found

## Tone of voice

- Calm, warm, honest and a little playful. Never alarmist, never pushy.
- It says what is measured, not what might be ("This folder holds 38 GB of installers you
  downloaded more than a month ago").
- It never promises speed or space it cannot prove. It celebrates only space that is
  really free (S10).
- Dutch uses the informal "je".

## Bubbles

| Type | Use | Example |
|---|---|---|
| Tip | A useful next step | "Click a cell to zoom in." |
| Explanation | What an item or a number means | "Hidden space is what Windows keeps out of reach…" |
| Progress | During a scan or a move | "Scanning… 412,000 files so far." |
| Warning | The purge is coming | "In 3 days I'll empty the batch from 31 October." |
| Celebration | Real freed space only | "12.4 GB is free again." |

### Rules

- **One bubble at a time, never a stack.** A newer message replaces an older one of the
  same or lower rank (warnings, then explanations, then celebrations and progress, then
  tips). A message of lower rank waits, and only the newest of each type waits. Warnings
  are never dropped (T-8.2).
- **Progress updates in place.** During a scan, the progress bubble changes its count
  every 2 seconds instead of adding bubbles, and it never interrupts another message.
- **Closing a bubble silences its topic** until that operation ends: close the progress
  bubble, and the rest of that scan stays quiet. A bubble that times out does not
  silence anything.
- **Never modal.** A bubble never blocks a click, never covers the item it talks about,
  and Esc dismisses it.
- **A question has buttons.** The one question the companion asks, whether it may read
  protected folders when a scan starts, carries two buttons. It is still a bubble: the
  scan runs on, and ignoring it means no.
- **No sound** by default.
- **Quiet mode** in Settings shows only warnings.
- Bubbles are announced through an `aria-live="polite"` region. A click on the orb opens
  a **history panel** with earlier bubbles (progress left out), so nothing important is
  lost; the next new bubble closes it.
- With reduced motion, the character stops its idle animation and bubbles fade instead of
  sliding.

## Triggers

| Trigger | What the companion says |
|---|---|
| `firstRun` | Hello, what Unhoard does, the privacy promise in one line, what it will scan first |
| `scanStart`, `scanDone` | Progress, then the top 3 findings |
| `hiddenSpace` | What hidden space is, and the offer to reveal it |
| `revealDeclined` | What stays unknown without admin, and that this is fine |
| `selected.<tier>` | What the tier of the selected item means |
| `quarantineMoved` | "Moved, but not freed yet": how the quarantine and the purge work |
| `purgeWarning.5`, `.3`, `.1` | The batch, its size and the purge date |
| `deleteEarly` | How to delete from the quarantine in Explorer (Shift+Delete, or Delete and empty the Recycle Bin) |
| `freeUpSpaceDone` | How much was really freed |
| `photoReview.*` | Tips for the review grid and the keyboard |
| `windowsTool.*` | Why a Windows tool is the right place for an item, with a link |

Script lines live in the locale files under `companion.<trigger>.<n>`. A trigger may have
several lines; one is chosen at random so the companion does not repeat itself. Every
trigger has English and Dutch text (T-8.1).

## AI chat

Ask Lumen questions about your storage in plain language: "Why is my disk full?", "What
grew this month?", "Which games haven't I played in a year?". The chat runs on a local
model through **Ollama**, the free app that runs AI models on your own PC. Nothing goes to
the internet.

### Switching it on

- The chat is **off by default**. Settings › Lumen › "Chat with a local model" turns it on.
- When switched on, Unhoard looks for Ollama at `http://127.0.0.1:11434` (only that
  address; a remote host is never used). Without Ollama, Lumen explains what it is and
  links to its download page in the browser; Unhoard downloads nothing itself.
- Unhoard lists the installed models and **recommends `qwen3:8b`**, which handled Dutch and
  tool calls best in the test during the build ([Decisions](24-decisions.md)).
  When the recommended model is missing, Lumen shows the `ollama pull <model>` command to
  copy, and "Open Ollama".
- The user may pick any installed model; a model that cannot call tools gets a warning.

### What the model sees

The model gets **tools**, never raw data. Each tool returns metadata from the current scan,
the history and the library:

| Tool | Returns |
|---|---|
| `overview` | Drive size, used, free and hidden space, and the top 10 folders of the scan |
| `children(path)` | The largest children of a folder, with size, kind and explanation |
| `explain(path)` | The explanation, tier, last use and actions of one item |
| `suggestions(group?)` | The suggestion groups with counts and sizes, or the items of one group |
| `growth(period)` | The largest growers over a day, week, month or three months |
| `search(name)` | Items whose name contains the text, at most 50 |
| `apps()` | Installed programs with size and last use; Steam games with last played |

- No tool returns file contents, and no tool changes anything. The chat can **point**:
  an answer may carry links to items, and a click selects the item in the overview or
  opens its group in Suggestions. Every move still goes through its preview and a click
  (S1).
- Paths are passed to the model as they are, because the model runs on this PC. They are
  never logged.

### The conversation

- A chat panel opens from the "Ask Lumen" button under Lumen on the overview. Replies
  stream in, and while the model calls a tool the panel says what Lumen is looking at.
- The system prompt says who Lumen is, that it answers in the user's language, that it
  only knows what the tools return, and that it never claims something is safe to delete
  without the tier and reason the tools give.
- A conversation lives in memory only and is gone when the app closes.
- A reply that takes longer than 60 seconds can be stopped; Ollama keeps running.

### Acceptance criteria

- AC8.6 The chat is off until switched on; when off, Unhoard opens no connection at all.
- AC8.7 Unhoard connects only to `127.0.0.1:11434`; any other host is refused in code.
- AC8.8 The tools return only the metadata listed above, never file contents, and no tool
  changes the disk or the settings.
- AC8.9 Without Ollama, or without the recommended model, Lumen explains the next step
  and Unhoard downloads nothing.
- AC8.10 Links in a reply select items or open groups; they never start a move.

| ID | Test | Kind |
|---|---|---|
| T-8.3 | the Ollama client refuses every host but 127.0.0.1:11434 | app unit |
| T-8.4 | every tool's output for a generated scan holds no file contents and stays under its size limit | core unit |
| T-8.5 | a tool call with an unknown name or bad arguments returns an error to the model, never a panic | app unit |
| T-8.6 | with the chat off, no connection is opened (fake transport) | app unit |

## Roadmap

- **Bring your own API key** for a cloud model, only after explicit consent.
- **Background notifications** from the tray icon when storage changes, together with
  [Optimise](16-optimise.md) background insights.

See [Roadmap](06-roadmap.md) and [Privacy](30-privacy.md).

---

## Related in this cluster

- [Storage map](10-storage-map.md): what the companion explains.
- [Cleanup and safety](11-cleanup-safety.md): the purge warnings and the Explorer guidance.
- [Photo review](14-photo-review.md): the review tips.
- [Optimise](16-optimise.md): the myths the companion explains.

Back to the [docs hub](README.md).
