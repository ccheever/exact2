# LLP 1060: Localized strings, `t("key", name=value)`

**Type:** RFC
**Status:** Implemented 2026-09-26
**Systems:** Compiler (`contract::strings`, `contract_types::strings`, `contract_lower::strings`); plan format (`locales` and `texts` tables, the `locale` header field, roster entry `t`; `exact_plan::strings`); runner (`Stdlib::T`, `Runner::set_place`, slot initialization order); `scripts/app.schema.json` (`strings.base`)
**Author:** Claude (Opus 5.5) for Seth Webster
**Date:** 2026-09-26
**Related:** LLP 1027.000.000 (`set_place`: the viewer's locale); LLP 1038 D2 (a compiler-made root slot the runner fills); LLP 1004 D4 (the roster); LLP 1005 §8 (dependency tracking); grnl's AGENTS.md §5 ("user-facing strings go through the string layer")

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
  A name is `[A-Za-z_][A-Za-z0-9_-]*`, what a Contract named argument can
  spell. Any other brace is literal text, so prose needs no escapes.
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
  | `strings-base` | an `app.json` `strings.base` that is not a tag |

  A translation may lag the base: a missing key reads the base's text at
  runtime. It may never lead the base. A key or placeholder the base lacks
  is a typo or a leftover that no `t` could show.
- *Rejected:* a warning for a key a translation lacks. The compiler has no
  warning level. Adding one for this would be the first piece of that
  apparatus, and a translation that is behind is a normal state, not a
  defect.
- *Rejected:* ICU MessageFormat (plurals, selects). No fixture needs it
  yet. `{count, plural, …}` is literal text under this grammar, so adding
  it later changes no accepted table's meaning, except where such text was
  meant literally.

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
  - `locales (name, texts: range)`: row 0 is the base, and the rest are in
    name order.
  - `texts (key, text)`: sorted by key within a locale, so lookup is a
    binary search.
- **Only the keys a `t` call names are baked.** An app that never calls
  `t` has no rows and no slot, even with a `strings/` directory. Keys only
  the TypeScript uses stay out of the plan.
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
  first `t` call lowers it, with the base as its initializer. This is the
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
`.json` sources, and the pinned `tsc` and Rolldown both accept JSON
imports under the bake's settings. The module receives the viewer's locale
the way it receives any fact: as an argument from Contract, read from
`exactTime`'s `locale` field. The runner's chain is ten lines, which the
app keeps beside its imports:

```ts
import en from './strings/en.json';
import fr from './strings/fr.json';

type Key = keyof typeof en;
const tables: Record<string, Partial<Record<Key, string>>> = { en, fr };

// The runner's chain (LLP 1060 D4): the longest subtag prefix, else the base.
function table(locale: string): Partial<Record<Key, string>> {
  for (let tag = locale; tag; tag = tag.slice(0, Math.max(tag.lastIndexOf('-'), 0))) {
    const name = Object.keys(tables).find(n => n.toLowerCase() === tag.toLowerCase());
    if (name) return tables[name];
  }
  return en;
}

export function t(locale: string, key: Key, values: Record<string, string | number> = {}): string {
  return (table(locale)[key] ?? en[key]).replace(/\{([A-Za-z_][A-Za-z0-9_-]*)\}/g,
    (whole, name: string) => (name in values ? String(values[name]) : whole));
}
```

```
shape Time
  locale: string
component App
  resource time = exactTime() as shape Time
  resource summary = weekSummary(time.locale) as shape Summary
```

- `keyof typeof en` checks keys at compile time in TypeScript as well.
- A changed locale changes the resource's arguments, so the source is
  asked again.
- *Rejected:* a runtime helper shipped by Exact2. The helper is ten lines
  an app can read. A shipped one would be a module every TypeScript app
  pays for, and a second place to keep in step with the runner.

## Unverified

- **Rust-app dev loop.** The Rust dev session (`host/web/src/dev.rs`)
  watches only `app.contract` and `.shells/surfaces.json`. An edit to
  `strings/*.json` shows at the next `app.contract` save. That session
  also misses edits to `use`d files. The TypeScript dev loop watches every
  captured source, JSON included.
- **Hosts.** No change was needed: Apple and the web already call
  `set_place` after boot. This was checked by reading the code, not on a
  device.
