[Unhoard](../README.md) › [Docs](README.md) › Trust › Threat model

# Threat model

What Unhoard protects, where untrusted data crosses into the app, what could go wrong,
and how each threat is mitigated. Mitigations map to the safety invariants S1–S10
([MVP scope](05-mvp-scope.md#safety-invariants)) and to the numbered engineering rules in
[AGENTS.md](../AGENTS.md) (for example rule 3.5 is "verify identity right before
acting"). Every Medium or higher change checks itself against this page, and updates it
when it adds an asset, a boundary or a threat.

## Assets

| ID | Asset | Why it matters |
|---|---|---|
| A1 | The user's files | Losing a photo or a document is the worst outcome |
| A2 | Cloud files and the cloud copy | A wrong action could download a library or delete it from the cloud |
| A3 | Windows and installed apps | Removing the wrong system file breaks the PC |
| A4 | Recovery information: manifests, the quarantine index, the activity log | Without it, a restore is impossible |
| A5 | Privacy: file names, paths, the user name, photos | A disk tool sees the whole PC |
| A6 | Free disk space | Hydrating a cloud library or copying instead of renaming could fill the drive |
| A7 | Privilege | The elevated helper must not become a way to gain admin |
| A8 | The user's trust in the numbers | A wrong "freed" number leads to wrong decisions |

## Trust boundaries

| ID | Boundary | Untrusted side |
|---|---|---|
| B1 | Webview → Rust (Tauri commands) | Everything the UI sends: IDs, options, strings |
| B2 | File system → scanner | File names, paths, attributes, reparse tags, sizes, timestamps |
| B3 | Windows APIs and the cloud client → platform layer | Thumbnails, property values, placeholder info, sync-root lists |
| B4 | Rule files and locale files → core | Patterns and text from contributors |
| B5 | Main app → elevated helper | The helper's arguments and its results over the pipe |
| B6 | Disk state between scan and action | Anything may change after the scan (time-of-check to time-of-use) |
| B7 | Other local accounts and processes | Anyone else on the PC who can read shared folders |
| B8 | Dependencies and the build pipeline | Crates, npm packages and GitHub Actions |

## Threats and mitigations

### Acting on the wrong file

| Threat | Mitigations |
|---|---|
| T1. The UI sends a path to delete or move outside what was scanned (B1, A1, A3) | Destructive commands take scan-index item IDs, never paths; Rust resolves and re-checks each ID; stale or unknown IDs are refused (rule 2.2, AC5.15, T-5.16) |
| T2. A file is swapped or changed between the scan and the move, for example a folder replaced by a junction to `C:\Windows` (B6, A1, A3) | Open with `FILE_FLAG_OPEN_REPARSE_POINT`, compare volume serial, 128-bit file ID, size and last-write time, then rename through the same handle; skip on a mismatch (rule 3.5, AC5.3, T-5.10) |
| T3. A rule or a bug offers a system folder for removal (B4, A3) | Protected paths are checked in the core on every resolved item; they never get a move action (S4, rule 2.6, T-4.3) |
| T4. A link inside the quarantine makes the purge delete outside it (B2, A1) | The purge works only inside a quarantine root, removes links as links and never follows them (S6, AC5.9, T-5.6) |
| T5. The purge deletes an item that is no longer what was moved (B6, A1) | The purge acts only on items whose manifest entry still matches the file (rule 3.3, T-5.11) |
| T6. Something is moved or deleted without the user asking (A1) | Nothing is pre-selected; every move is a click; the purge is the only automatic action and waits for a shown warning (S1, AC5.8, T-5.5) |

### Losing recovery information

| Threat | Mitigations |
|---|---|
| T7. A crash or power loss in the middle of a move (A4) | Write-ahead manifest, flushed before the move; reconcile `moving` entries at start-up (S5, rule 3.4, T-5.1, T-5.4) |
| T8. A full disk stops the manifest write (A4, A6) | The move does not start if the manifest cannot be written (T-5.12) |
| T9. A restore overwrites a newer file (A1) | Restore never overwrites; it uses `name (restored).ext` (rule 3.9, T-5.3) |
| T10. A batch partly fails and the user loses track (A4) | Per-item results; recovery information is never dropped; every operation is in the activity log (rules 3.10, 3.11, T-5.7, T-5.15) |

### Cloud

| Threat | Mitigations |
|---|---|
| T11. A thumbnail, hash or open hydrates an online-only file and fills the disk (B3, A2, A6) | No `FILE_READ_DATA` open inside a sync root; the fake records every open and fails the test on a data open; real runs re-check allocation sizes and stop the photo pipeline on hydration (S2, rule 3.7, T-6.2) |
| T12. Moving cloud files to the local quarantine downloads them and deletes them from the cloud (A2) | Cloud items never offer the local quarantine (S3, T-6.3) |
| T13. "Free up space" on a file that is not uploaded yet loses its only copy (A2) | Only files that are in sync are offered (AC6.4, T-6.4) |
| T14. The cloud quarantine move turns into a download plus upload (A2, A6) | A rename inside the same sync root through a handle without data access (T-6.5); confirmed on a real client in the manual checks |

### Privacy

| Threat | Mitigations |
|---|---|
| T15. Data leaves the PC (A5) | No connection to another computer; no HTTP client in the build; the Ollama client connects only to 127.0.0.1:11434 and only while the chat is on; no telemetry, ever (S8, T-S8, T-8.3) |
| T32. A file name tricks the chat model into harmful advice (prompt injection) (B2, A1) | Tools are read-only and the chat can only select items; every move needs its preview and a click (S1); answers about safety must quote the tier and reason from the tools |
| T33. Another local program poses as Ollama on port 11434 (B7) | It receives only questions and metadata the user chose to send; replies are shown as text and links that only select; nothing from a reply is executed |
| T34. The history file reveals folder names to another account (A5, B7) | It lives in the user's own `%LOCALAPPDATA%`, which other standard accounts cannot read; it holds no names below the largest folders |
| T35. The scheduled task is changed to run something else (B7) | It is created for the current user and points to Unhoard's own executable with a fixed argument; any program of the same user could already run code, so the task adds no new privilege; turning insights off removes it |
| T36. The startup toggle disables something the user needs (A3) | It writes only the value Task Manager writes, previews first, undoes with one click and logs the change; machine-wide entries are not touched |
| T16. A shared export, CLI output or bug report shows the user name or private paths (A5) | `%USERPROFILE%` redaction with a test; logs hold no paths unless diagnostic logging is on (S9, rule 2.7, T-10.3) |
| T17. Other local accounts read items moved from a private profile into `X:\Unhoard Quarantine\`, which inherits the drive root's permissive ACL (B7, A5) | The quarantine folder gets an explicit, non-inherited ACL for the current user, SYSTEM and Administrators only, set when it is created and checked before every move |
| T18. Background insights become a hidden record of the user's files (A5) | They store only sizes of the largest folders, no names below them and no contents; local only; limited to 20 MB and 2 years; switchable off and clearable ([Optimise](16-optimise.md#background-insights)) |

### Malicious content on disk

| Threat | Mitigations |
|---|---|
| T19. A crafted file name or path breaks parsing or the UI (B2) | Names are handled as UTF-16 with the `\\?\` form; limits on path length, depth and counts; the UI renders names as text, never as HTML (rule 3.6) |
| T20. A crafted image exploits a decoder (B3) | Unhoard decodes no full images; thumbnails come from Windows with size and dimension limits (rule 2.5) |
| T21. Opening a found file runs code (B2) | Scans read metadata only; "Open" and "Show in Explorer" happen only on an explicit click (rule 2.5) |
| T22. A junction loop or a deep tree exhausts memory or hangs the scan (B2) | Links are not followed; depth and entry limits; bounded worker pool; cancellation (AC1.5, T-1.3) |

### Privilege

| Threat | Mitigations |
|---|---|
| T23. The elevated helper is abused to change files or gain admin (B5, A7) | The helper is read-only and has no write command; it accepts only validated absolute local paths, at most 4,096, checked again for every request; it lives only as long as the app's pipe, so it exits when the app quits; the main app never runs elevated (S7, rule 2.4) |
| T24. Another process connects to the helper's pipe or impersonates it (B5) | The pipe is local-only, single-instance, with a random name; the app sends the folder list only after checking that the client is the helper process it started |
| T26. A local tool that an adapter runs is replaced or fed hostile input (B4) | Tools run only after a click, from the app's own install folder, never from `PATH` or a scanned path; fixed read-only arguments and no shell; time and output limits; output parsed as untrusted data |
| T27. Registry or app records (installed programs, UserAssist, Steam, Ollama) hold hostile values (B4) | Read-only; size limits on every value; parsed as data, never executed; paths only compared, never opened from these records |
| T25. The webview is compromised and calls powerful APIs (B1) | Tauri capabilities allow only Unhoard's commands; strict CSP; no remote content; no broad `fs` or `shell` scope (rule 2.3) |

### Supply chain

| Threat | Mitigations |
|---|---|
| T26. A malicious or vulnerable dependency (B8) | Every dependency has a written reason; `cargo deny` for licenses and advisories; `npm audit`; committed lockfiles; `npm ci` in CI (rule 8) |
| T27. A tampered GitHub Action (B8) | Actions pinned to full commit SHAs; Dependabot updates them (rule 8) |
| T28. A secret reaches the repository (B8) | gitleaks in CI on every push and weekly; a secret in history counts as leaked |
| T29. A fake download of Unhoard carries malware (A1, A5) | Official downloads only from GitHub Releases and winget, with SHA-256 checksums; the README warns about other sources; code signing in Next |

## Trust in the numbers

| Threat | Mitigations |
|---|---|
| T30. The app reports space as freed that is not free (A8) | Freed space is reported only after a re-measure; a quarantine move never shows as freed (S10, AC5.7, T-5.8) |
| T31. An unknown size shows as 0 (A8) | Unknown is `null` or "unknown", never 0 (rule 4, AC10.7) |

## Out of scope

- An attacker who already runs code as the same user or as admin. They can do anything
  Unhoard can do.
- Physical access to an unlocked PC.
- Vulnerabilities in Windows, WebView2 or the OneDrive client themselves. Unhoard limits
  how much it relies on them, and reports issues upstream.

---

## Related in this cluster

- [Privacy](30-privacy.md): the promises this model protects.
- [Glossary](31-glossary.md): the terms used here.

Back to the [docs hub](README.md).
