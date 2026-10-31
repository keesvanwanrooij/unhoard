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
- **Size:** about 96 px in the corner of the stage, never over the item it talks about.
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

- **One bubble at a time**, from a priority queue: warnings first, then explanations,
  progress and tips (T-8.2).
- **Never modal.** A bubble never blocks a click, never covers the item it talks about,
  and Esc dismisses it.
- **No sound** by default.
- **Quiet mode** in Settings shows only warnings.
- Bubbles are announced through an `aria-live="polite"` region, and a **history panel**
  lists earlier bubbles, so nothing important is lost.
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

## Roadmap

- **AI chat with a local model.** Ask questions about your storage in plain language.
  - Candidates: Gemma 4 E4B or Qwen3.5 4B, about 3 GB as GGUF, run through llama.cpp. The
    choice depends on a test of Dutch quality and tool-call reliability. Models below
    about 2B parameters are too weak.
  - The model is downloaded only when you ask for it, never bundled.
  - It sees only metadata from the scan index, through tools. It never reads file
    contents unless you explicitly allow it.
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
