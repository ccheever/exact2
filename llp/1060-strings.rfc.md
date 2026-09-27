# LLP 1060: Localized strings, `t("key", name=value)`

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Compiler (`contract::strings`, `contract_types::strings`, `contract_lower::strings`); plan format (`locales` and `texts` tables, the `locale` header field, roster entry `t`; `exact_plan::strings`); runner (`Stdlib::T`, `Runner::set_place`, slot initialization order); `scripts/app.schema.json` (`strings.base`)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1027.000.000 (`set_place`: the viewer's locale); LLP 1038 D2 (a compiler-made root slot the runner fills); LLP 1004 D4 (the roster); LLP 1005 §8 (dependency tracking); grnl's AGENTS.md §5 ("user-facing strings go through the string layer")


**Ruled (Charlie, 2026-09-27, after the review of PR #47):**
- Tables are written in **Unicode MessageFormat 2** syntax, the basis of the TC39 `Intl.MessageFormat` proposal. Only `{name}` placeholders are implemented at first. Every other MF2 construct (`.match`, plural and select, functions, markup) is refused at compile time by name until it is built, never treated as literal text, so no table accepted today changes meaning when plurals arrive.
- The resolved table's locale sets `lang`, and its direction sets `dir`, on every host (the web's document element; the native accessibility language and layout direction).
- `exactTime` carries the resolved locale, so a TypeScript source reads it instead of re-running the RFC 4647 lookup.

## Summary

An app keeps its UI text in `strings/<locale>.json` beside `app.contract`.
Contract reads it with `t`:

```
text t("entries.title") testId="title"
text t("entries.count", count=length(entries), name=user.name)
```

```json
// strings/en.json
{ "entries.title": "Journal", "entries.count": "{count} entries, {name}" }
```

- The compiler checks every call against the base table and bakes the
  tables into the plan.
- The locale is a slot. When the host reports the viewer's locale, the
  runner writes the resolved table's name into that slot, and ordinary
  dependency tracking re-renders exactly the texts `t` produced.
- The app's TypeScript imports the same JSON files.

## Motivation

grnl requires every user-facing string to go through a string layer.
Contract had none, so grnl's UI strings were literals in `.contract` files
and its data module's strings were literals in TypeScript. The host already
reports the viewer's locale (`set_place`); nothing used it for text.

## Design

### D1 — Tables are flat JSON files, one per locale

- **Location:** `strings/<tag>.json` in the app directory. `<tag>` is a
  BCP 47 tag: `en`, `en-GB`, `pt-BR`. Non-JSON files in `strings/` are
  ignored.
- **Contents:** a flat object of key → text. `{name}` is a placeholder.
  Values are MF2 simple messages. Both `{$name}` (MF2's variable spelling)
  and `{name}` (the Contract shorthand Charlie requested) interpolate a name
  spelled `[A-Za-z_][A-Za-z0-9_-]*`. Whitespace inside the braces is allowed.
  `\{`, `\}`, and `\\` escape text; other escapes or unescaped braces refuse.
  `.match`, `.input`, `.local`, functions, markup, attributes, literals,
  quoted patterns, and other expressions refuse by name (`strings-message`),
  including in tables no Contract call reads. The shorthand is the only
  departure from [MF2 simple-message syntax](https://messageformat.unicode.org/docs/quick-start/).
- **Base:** `app.json`'s `strings.base`, or `en` when it names none. Every
  `t` is checked against the base table, and every other table falls back
  to it.
- **Checked on every compile,** whether or not a `t` reads the tables. The
  app's TypeScript reads them too. Every refusal is reported in one run,
  naming the file:

  | id | refused |
  | --- | --- |
  | `strings-locale` | a file name that is not a tag; two tables for one tag (case-insensitive) |
  | `strings-table` | anything but a flat object of strings |
  | `strings-base-missing` | no table for the base locale |
  | `strings-unknown-key` | a translation's key the base table lacks |
  | `strings-placeholder` | a translation's placeholder its base text lacks |
  | `strings-message` | unsupported MF2 construct or malformed message, naming the key |
  | `strings-base` | an `app.json` `strings.base` that is not a tag |

  A translation may lag the base: a missing key reads the base's text at
  runtime. It may never lead the base. A key or placeholder the base lacks
  is a typo or a leftover that no `t` could show.
- *Rejected:* a warning for a key a translation lacks. The compiler has no
  warning level. Adding one for this would be the first piece of that
  apparatus, and a translation that is behind is a normal state, not a
  defect.

### D2 — `t("key", name=value)`: a literal key, named placeholders

- **Syntax:** `t` takes a key and named arguments, in the existing
  named-argument syntax `name=value`. The parser already refuses
  `name: value` there. Each value is a number, a string, or a bool, as a
  template part is.
- **Refusals** (types pass):

  | id | refused |
  | --- | --- |
  | `type-strings-key` | a key that is not a string literal |
  | `type-strings-unknown-key` | a key the base table lacks, with a spelling suggestion |
  | `type-strings-placeholder` | a base placeholder with no argument |
  | `type-strings-argument` | an argument the base text does not use; a positional, repeated, or non-scalar argument |
  | `type-strings-missing` | a `t` call with no tables. `compile(src)` has no directory, so it has none |

- **Name resolution:** `t` is a roster entry, so `fn t` is refused as
  shadowing it. An action or action prop named `t` is still called as that
  action.
- **Lowering:** `t` lowers to the roster call `t(locale, key, pairs)`:
  1. the locale slot;
  2. the key;
  3. a `list<string>` of name/value pairs, with non-strings through
     `toString`.

  The runner fills the placeholders of the resolved table's text by name,
  so a translation may order them differently.
- *Rejected:* a new opcode. A roster call reads through the same
  `LoadSlot` that makes dependency tracking work, and costs no VM change.
- *Rejected:* passing values positionally in base order. The runner would
  then need each table's placeholder order baked beside the text, which is
  more plan data for no saving.

### D3 — The plan bakes only what `t` names

- **Tables:**
  - `locales (name, rtl, texts: range)`: row 0 is the base, and the rest are in
    name order.
  - `texts (key, text)`: sorted by key within a locale, so lookup is a
    binary search.
- **Only the keys a `t` call names are baked.** An app that never calls
  `t` has no text rows. Locale names, CLDR script directions and the slot
  remain when tables exist, so data-only translations still resolve through
  `exactTime`. Keys only the TypeScript uses stay out of the plan.
- **Validation** (`Plan::validate_texts`):
  - the slot and the tables come together;
  - the slot is a root `string` slot;
  - locale names are unique and non-empty;
  - each table is strictly sorted.
- The plan remains a pure function of the sources, now including
  `strings/*.json`.

### D4 — The locale is a compiler-made slot the runner writes

- **The slot:** the header field `locale` names a root slot, `#locale`.
  The lexer cannot produce the name, so no app name collides with it. The
  first `t` call lowers it, with the base as its initializer; table-only apps
  receive it when the compiler bakes the table names. This is the
  router's model (LLP 1038 D2): a value the runner owns, read through
  ordinary slot dependencies.
- **Boot:** the slot is filled first, so a state initializer may call `t`.
  Bake and boot therefore show the base, and bake output is
  deterministic.
- **`set_place(locale, zone)`** also resolves the tag and writes the result
  into the slot, in the same single commit as the `exactTime` re-answer.
  Resolution is RFC 4647 lookup:
  1. the longest subtag prefix a table is named for, compared without
     case (`zh-Hant-TW`, then `zh-Hant`, then `zh`);
  2. otherwise the base.

  This extends the brief's "exact, then language, then base" to three or
  more subtags; for `en-GB` it is the same.
  - A place that resolves to the table already shown writes nothing. With
    no `exactTime` resource, it commits nothing.
  - A refused commit restores both the place and the slot.
  - The resolved table supplies HTML `lang` and `dir` on the web document
    element and rendered pages, native accessibility language and direction,
    and the Linux shaper's locale. Direction is compiled with ICU4X's CLDR
    likely-subtags and script-direction data; explicit scripts override the
    language default. The locale data is a compiler dependency only. Native
    layout inherits this document direction beneath authored CSS overrides.
    Language changes invalidate native text measurement even for fixed text.
    Without tables, `lang` and `resolvedLocale` are empty (unknown), and `dir`
    is `ltr`.
- **Lookup:** `t` reads the text for (slot, key), or the base's when that
  table lacks the key. A key missing from the base traps. The compiler
  proved that key exists, so this happens only in a hand-built plan.
- **Carry:** a dev reload keeps the slot like any slot. The page keeps its
  language instead of flashing the base, and the host's next `set_place`
  resolves to the same table.
- *Rejected:* resolving in `t` at every call from a runner-held place. The
  runner would have to invalidate every `t` reader by hand. A slot gets
  that from dependency tracking and shows in `state` for the agent.
- *Rejected:* excluding the slot from carry, as the router's is excluded.
  The router slot is excluded because its value decodes through the old
  plan's shapes. A locale is a plain string.

### D5 — TypeScript imports the same files

A data module imports the tables it needs. The TypeScript bake captures
`.json` sources. Read `exactTime.resolvedLocale` for table selection;
`exactTime.locale` remains the viewer's locale for date and number formatting.
Table names retain their authored case, so indexing needs no lookup chain:

```ts
import en from './strings/en.json';
import fr from './strings/fr.json';

type Key = keyof typeof en;
const tables: Record<string, Partial<Record<Key, string>>> = { en, fr };

export function t(resolvedLocale: string, key: Key, values: Record<string, string | number> = {}): string {
  return (tables[resolvedLocale]?.[key] ?? en[key]).replace(
    /\\([{}\\])|\{\s*\$?([A-Za-z_][A-Za-z0-9_-]*)\s*\}/g,
    (whole, escaped: string, name: string) => escaped ?? (name in values ? String(values[name]) : whole));
}
```

```
shape Time
  locale: string
  resolvedLocale: string
component App
  resource time = exactTime() as shape Time
  resource summary = weekSummary(time.resolvedLocale) as shape Summary
```

`resolvedLocale` is the same slot Contract's `t` reads, including at bake and
boot and during a carried reload. A locale change re-answers `exactTime` in
the same commit. The in-repo apps have no copied lookup to remove as of
2026-09-27; this example previously contained the duplicate chain.

## Unverified

- **Rust-app dev loop.** The Rust dev session (`host/web/src/dev.rs`)
  watches only `app.contract` and `.shells/surfaces.json`. An edit to
  `strings/*.json` shows at the next `app.contract` save. That session
  also misses edits to `use`d files. The TypeScript dev loop watches every
  captured source, JSON included.
- Host verification for the 2026-09-27 ruling is recorded in
  `issues/20260927-strings-messageformat-and-lang.md`.
