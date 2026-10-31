## What and why

<!-- What this change does and why. Link the issue: "Closes #…". -->

## Risk class

<!-- Low, Medium, High or Critical (AGENTS.md › Change risk classes). When unsure, pick the higher one. -->

Risk:

Feature and invariant IDs (F1–F11, S1–S10):

## Checks

- [ ] Tests were written first and seen failing (Medium and higher)
- [ ] `cargo fmt --all --check`, `cargo clippy … -D warnings`, `cargo test --workspace`, `cargo deny check`
- [ ] `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`
- [ ] Performance: `cargo bench -p unhoard-core`, `npm run perf`, `npm run perf:check` (for UI, core hot paths or IPC; numbers below)
- [ ] Docs updated in this change (the hub, `llms.txt` and the README for a new doc)
- [ ] No personal data, real paths, real scans or secrets in the diff

## Threat check (Medium and higher)

<!-- The asset at stake, the trust boundary crossed, the worst case, the mitigation. -->

## Security review or safety argument (High and Critical)

<!-- High: a self-review of the diff against AGENTS.md rule 2. Critical: the safety argument, the dry-run test and the audit-log test. -->

## AI assistance

<!-- Which parts were AI-assisted, if any. You have read and understood every line. -->

## Known limitations
