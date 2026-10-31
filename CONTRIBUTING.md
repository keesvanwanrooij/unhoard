# Contributing to Unhoard

Thank you for helping. Unhoard handles people's files, so every contribution follows the
same safety rules, whoever writes it. Read [AGENTS.md](AGENTS.md) for the full
engineering standards and the [docs hub](docs/README.md) for how the app works.

## Ways to help

- **Bug reports** and **feature ideas**: use the issue templates.
- **Known-location rules**: explain a big folder or file that Unhoard does not know yet.
  See [Known locations](docs/12-known-locations.md#adding-a-rule).
- **Translations**: improve the Dutch text, or propose a new language with the
  translation template ([Translations](docs/23-translations.md)).
- **Code**: pick an issue, say you are working on it, and open a pull request.

Please do not attach real scans, real paths with your user name, or real photos to an
issue. Replace your profile path with `%USERPROFILE%`.

## Before you start

1. Set up the tools in [Development](docs/22-development.md#setup).
2. **Classify the change:** Low, Medium, High or Critical
   ([risk classes](AGENTS.md#change-risk-classes)). When unsure, pick the higher class.
3. For anything that changes behaviour, architecture or the UI, find the doc it belongs
   to. The doc is updated in the same pull request.

## Test first

For every Medium or higher change:

1. Write a test that describes the behaviour and **see it fail**.
2. Write the code until it passes.
3. Refactor while it stays green.

A bug fix starts with a test that reproduces the bug. Test names carry the feature or
invariant ID (`s5_manifest_written_before_move`). Test data is generated in temp folders,
never a real disk or real photos. Details:
[Development](docs/22-development.md#test-driven-development).

## Checks

Run these before you open a pull request:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
npm run typecheck
npm run lint
npm test
npm run build
```

## Commits

- [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`,
  `test:`, `refactor:`, `chore:`.
- The body names the risk class and the F or S IDs it touches:

  ```text
  feat: skip junctions that point outside the scan root

  Risk: medium. F1, S6.

  Signed-off-by: Your Name <you@users.noreply.github.com>
  ```

- **Every commit is signed off** for the Developer Certificate of Origin.

### Developer Certificate of Origin

By signing off a commit you state that you wrote the change, or otherwise have the right
to submit it under the project's license, as described in the
[Developer Certificate of Origin](https://developercertificate.org/). Add the sign-off
with:

```powershell
git commit -s
```

There is no contributor license agreement. Your contribution stays yours, licensed under
GPL-3.0-or-later.

## AI-assisted contributions

AI coding assistants are welcome, on these terms:

- **Disclose it.** Say in the pull request which parts were AI-assisted.
- **You are the author.** You have read and understood every line, and you sign off the
  DCO for it.
- **Tests carry the trust.** AI-assisted code meets the same test-first rule, checks and
  review as any other code.
- Never paste private data, secrets or real file listings into an AI tool for a
  contribution.

## Pull requests

- One topic per pull request, with the template filled in.
- CI must be green.
- High and Critical changes include the security review or safety argument the
  [risk class](AGENTS.md#change-risk-classes) requires. Critical changes wait for the
  maintainer's review.

## Conduct and security

- Everyone follows the [Code of Conduct](CODE_OF_CONDUCT.md).
- Report vulnerabilities privately, never in a public issue: see [SECURITY.md](SECURITY.md).
