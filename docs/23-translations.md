[Unhoard](../README.md) › [Docs](README.md) › Engineering › Translations

# Translations

Unhoard speaks English and Dutch. This doc covers how the language is chosen, where the
text lives, how numbers are formatted, the style per language and the terms we use.
Acceptance criteria: F9 in [MVP scope](05-mvp-scope.md).

## Choosing the language

- At start, Unhoard reads the Windows display languages with
  `GetUserPreferredUILanguages`.
- `nl-*` (Dutch in the Netherlands or Belgium) → Dutch. Everything else → English.
- A choice in Settings overrides this and is remembered.

## Where text lives

- `src/i18n/en.json` and `src/i18n/nl.json`, with namespaces per feature (`map`,
  `quarantine`, `cloud`, `photos`, `companion`, `knownLocations`, `settings`).
- Plurals use ICU-style messages through i18next:
  `"{count, plural, one {# file} other {# files}}"`.
- Known-location texts: `knownLocations.<id>.title`, `.explain`, `.reason`, `.action`
  ([Known locations](12-known-locations.md)).
- Companion lines: `companion.<trigger>.<n>` ([Companion](15-companion.md)).
- No UI text is hard-coded (lint rule). English and Dutch have exactly the same keys
  (T-9.1).
- Rust returns message keys and parameters, never finished sentences, so all user text
  is translated in one place.

## Numbers, sizes and dates

- `Intl.NumberFormat` and `Intl.DateTimeFormat` with the UI language: "1.5 GB" in
  English, "1,5 GB" in Dutch.
- **Units are binary and labelled as Explorer labels them:** 1 KB = 1024 bytes,
  1 GB = 1024³ bytes, shown as "KB", "MB", "GB" and "TB". The numbers therefore match
  what Explorer and Settings show.
- Sizes show three significant digits (12.4 GB, 868 GB). An unknown size is "unknown"
  ("onbekend"), never 0.

## Style

### English

- Plain, short sentences. Say what to do next.
- Sentence case for buttons and headings.
- British spelling where it differs ("optimise", "colour"), because the docs use it.

### Dutch

- Informal "je", never "u".
- Use the words Windows NL itself uses for Windows features, so users recognise them.
- No anglicisms where Dutch has a common word ("bestand", not "file"), but keep product
  names as they are (OneDrive, WinSxS).

## Terms

### Windows terms

These follow Windows in Dutch (verify each against a current Windows 11 NL build).

| English | Dutch |
|---|---|
| Recycle Bin | Prullenbak |
| File Explorer | Verkenner |
| Settings | Instellingen |
| Storage | Opslag |
| Storage Sense | Opslaginzicht (verify) |
| Disk Cleanup | Schijfopruiming |
| Free up space | Ruimte vrijmaken |
| Always keep on this device | Altijd behouden op dit apparaat |
| Task Manager | Taakbeheer |

### Product terms

| English | Dutch |
|---|---|
| quarantine | quarantaine |
| hidden space | verborgen ruimte |
| cell map | celkaart |
| treemap | treemap |
| Safe | Veilig |
| Likely safe | Waarschijnlijk veilig |
| Review | Bekijken |
| Keep (system) | Behouden (systeem) |
| not freed yet | nog niet vrijgemaakt |
| Optimise | Optimaliseren |

The [Glossary](31-glossary.md) explains every term.

## Testing text

- A lint rule refuses hard-coded text in JSX, so every visible string goes through i18n.
- Tests check that both languages have the same keys, that every companion trigger and
  every known-location rule has text in both, and that the language follows Windows with
  an English fallback (T-8.1, T-4.2, T-9.1, T-9.2).

## Adding a language

More languages come after the English and Dutch texts are stable ([Roadmap](06-roadmap.md)).
Use the translation issue template to propose one. A new language needs every key, the
Windows terms from that Windows language, and a native speaker who reviews it.

---

## Related in this cluster

- [Architecture](20-architecture.md): how message keys travel from Rust to the UI.
- [Development](22-development.md): the checks that guard the text.
- [Decisions](24-decisions.md): why units and language selection work this way.
- [Tech stack](21-tech-stack.md): i18next and react-i18next.

Back to the [docs hub](README.md).
