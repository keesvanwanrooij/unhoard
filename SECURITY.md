# Security policy

Unhoard moves files, reads cloud folders and can start an elevated helper, so security
reports matter to us. Thank you for reporting responsibly.

## Reporting a vulnerability

**Do not open a public issue.** Report it privately through GitHub:

1. Go to the repository's **Security** tab.
2. Choose **Report a vulnerability**.
3. Describe the problem, the steps to reproduce it, and the impact you expect.

This uses GitHub's private vulnerability reporting, so only the maintainer sees the
report. Please do not include real scans, personal paths or real photos; a generated
reproduction is enough.

## What to expect

- An acknowledgement within 7 days.
- An assessment and a plan within 30 days.
- Credit in the advisory and the release notes, if you want it.

Unhoard is maintained by one person in their spare time, so these are targets, not
guarantees.

## In scope

Especially, but not only:

- **Quarantine bypass:** anything deleted or moved outside the quarantine flow, or
  deleted before the 30-day purge.
- **Protected paths:** any way to make Unhoard move or delete Windows, program folders,
  profile roots or drive roots.
- **Wrong-file actions:** acting on a file other than the one the user chose, for example
  through a swapped file or a link.
- **Cloud hydration:** any path that downloads an online-only file, or deletes cloud
  content outside the cloud quarantine purge.
- **The elevated helper:** any way to make it write, or to use it to gain privileges.
- **Network traffic:** any connection the app opens without the user switching a feature
  on.
- **Data leaks:** personal paths or file names in exports, logs or errors where the docs
  say they are redacted.
- **The webview:** script injection through file names or metadata, or calls to commands
  the UI should not reach.

The [threat model](docs/32-threat-model.md) lists the threats we already defend against.

## Out of scope

- Attacks that need the attacker to already run code as the same user or as admin.
- Vulnerabilities in Windows, WebView2 or the OneDrive client themselves (please report
  those upstream).
- SmartScreen warnings on unsigned releases (signing is on the
  [roadmap](docs/06-roadmap.md)).

## Supported versions

Only the latest release receives security fixes.
