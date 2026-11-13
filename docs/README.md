[Unhoard](../README.md) › Docs

# Unhoard documentation hub

This is the map of every Unhoard document. The docs are the source of truth: if the
code and a doc disagree, one of them is fixed in the same change.

New here? Read these three first:

1. [Vision](01-vision.md): why Unhoard exists and the principles it follows.
2. [MVP scope](05-mvp-scope.md): what the first build contains, with testable acceptance
   criteria.
3. [Architecture](20-architecture.md): how the pieces fit together.

## Product

What we build, for whom, and why.

- [Vision](01-vision.md): the problem, the promise and the principles behind Unhoard.
- [Target users](02-target-users.md): who Unhoard is for, with the ideal user profile and
  four personas.
- [Market landscape](03-market-landscape.md): existing open-source, paid and built-in
  tools, with a feature matrix.
- [Competitive advantage](04-competitive-advantage.md): where Unhoard is better, where it is
  not, and why.
- [MVP scope](05-mvp-scope.md): the first build, with acceptance criteria and the tests that
  come first.
- [Roadmap](06-roadmap.md): Now, Next and Later, without version numbers.

## Features

How each part of the app behaves.

- [Storage map](10-storage-map.md): the living cell map, the treemap switch, colours, zoom
  and hidden space.
- [Cleanup and safety](11-cleanup-safety.md): risk tiers, protected paths, the quarantine
  and the 30-day purge.
- [Known locations](12-known-locations.md): the library that explains big items and says
  what to do with them.
- [Cloud storage](13-cloud-storage.md): OneDrive placeholders, local versus cloud size, and
  freeing up space without downloads.
- [Storage history](17-storage-history.md): snapshots over time, the growth timeline and
  what grew, and why.
- [Photo review](14-photo-review.md): duplicates, bursts, chat media and screenshots,
  reviewed per group without downloading.
- [Companion](15-companion.md): the guide character, its speech bubbles and its script.
- [Optimise](16-optimise.md): the honest check-up of startup, disk health, Windows health
  and resource use, with safe reversible actions and no speed promises.

## Engineering

How it is built and how to work on it.

- [Architecture](20-architecture.md): the Rust core, the Tauri shell, the React UI, the CLI
  and the elevated helper.
- [Tech stack](21-tech-stack.md): the choices we made and the alternatives we rejected.
- [Development](22-development.md): setup, build, test-driven workflow, comment standard and
  releases.
- [Translations](23-translations.md): English and Dutch, language selection, style and
  terminology.
- [Decisions](24-decisions.md): every current decision with its reason, plus open
  questions.
- [Performance](25-performance.md): the speed budgets that keep the app pleasant, such as
  screen switches within 100 ms, and the tests that catch a slowdown.

## Trust

What Unhoard promises about your data, how it stays safe, and the words we use.

- [Privacy](30-privacy.md): no telemetry, no network by default, and what stays on your PC.
- [Glossary](31-glossary.md): the terms used in the app and the docs, with Dutch
  equivalents.
- [Threat model](32-threat-model.md): assets, trust boundaries, threats and mitigations,
  mapped to the safety invariants.

## How these docs are organised

- One flat folder, no subfolders. File names are a two-digit number plus a kebab-case
  name, so the folder lists in reading order. The first digit is the cluster: `0x`
  Product, `1x` Features, `2x` Engineering, `3x` Trust.
- A new doc takes the next free number in its cluster. Existing docs are never
  renumbered, because that would break links. This hub stays `README.md`.
- Every doc starts with a breadcrumb (`Unhoard › Docs › Cluster › Title`) and ends with
  "Related in this cluster" plus a link back to this hub.
- No version numbers. The roadmap uses Now / Next / Later.
- When you add or rename a doc, update this hub, [`llms.txt`](../llms.txt) and the links
  in the root [README](../README.md) in the same change. CI checks that all three list
  the same docs.

## Outside the docs folder

- [README](../README.md): the project front page.
- [AGENTS.md](../AGENTS.md): onboarding for contributors and AI agents.
- [CONTRIBUTING.md](../CONTRIBUTING.md): how to contribute, with the DCO sign-off.
- [SECURITY.md](../SECURITY.md): how to report a vulnerability.
- [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md): how we treat each other.
