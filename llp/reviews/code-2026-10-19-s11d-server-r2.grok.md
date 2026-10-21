# Code review: the server half of stage 11 D (base names on the wire, tamber/1.14), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b18206c78e`.
- **Method:** one brief (sha256 `29e947196cbdfed9d611e1b1c2d0859ac215b215cc3e7664bd5ce571632be87e`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this wire-change commit read-only: both commit messages, the round-1 findings, and the base-row shape against the brief and the code.HEAD is the round-1 follow-up. I'll read both findings, the brief, and the wire diff before checking the shape against the kernel.Round-1 findings are in hand. I'll read the current wire path and the checker it now depends on, then match each finding to a fix or an answer.One blocker: `explain`'s `bases` drops an unset base when that name sits on a value no tuple selects. The row shape, override detection, and `targets` match the brief. `tamber_serve_base` (2) and kernel `scale` (10) both passed; neither covers this case.

## Blocker

**An unset base on a value no tuple selects never appears in `bases`.** `contract/cli/src/tamber_base.rs:77`

`bases_of` keeps a name only when some selected tuple folded to `Length::Named`, or when `token.names` lists that base token. A bare unset base never becomes a ref: `tamber/src/measure.rs:222` returns `Length::Named` and does not call `Names::named`. When every tuple hits a `when`, `unselected` (`tamber/src/check.rs:977`) still folds the default and throws the value away. The syntax requires that default, so this is a normal design:

```text
card.pad = space.md
  when density=roomy: 12px
  when density=tight: 8px
```

The design checks. `explain` returns `value: "space.md"` and omits `bases`. The same drop hits an overridden base whose own default names another unset base (`space.xl = space.lg` with a `when` for every mode), and a transitive default (`card.pad = other`, `other = space.md`) whose folded length is that named base. A set base on that same default is recorded, because `unselected` does update `refs`.

**Fix:** On every `fold_kind` result, including the one `unselected` discards, if the value is `Length::Named` and the name is a base, store it on the token (default, then `when`s, first time wins). `bases_of` should emit that list, unioned with `token.names` that are base names. Add this design beside `card.pad`.

## Should-fix

**`resolve` and `explain` still join two reads.** `contract/cli/src/tamber_base.rs:137` and `:166`

`describe` re-reads and continues while `files` disagree (`tamber_base.rs:96-100`). An overridden `resolve` takes `base` from `as_base`'s fold and the tuples from a second `s.resolve`. A non-base `explain` takes the body from `s.explain` and `bases` from a third `lookup`. A save in between pairs a new `value` with the previous `override` or `bases`, and `files` belongs to only one of those reads. Every `resolve` parses the design twice; every non-base `explain` parses it three times.

**Fix:** Build the tuples, the explain fields, and `bases` from the `Snapshot` and `Folding` the first `lookup` already returned.

No stable request that previously succeeded now fails. A non-base token, a param (a component `space`'s `md` resolves to the param, because `as_base` requires the looked-up name to equal the request), an icon, and a `file` request all use `lookup` / `design_with` and surface that path's error. A missing file and a refused design fail for a base name too. `describe` answers "the design kept changing while it was read" only after three mismatched snapshots.

## Nit

**Column keys are alphabetical.** `contract/cli/src/tamber_base.rs:42`

`serde_json::Map` is a `BTreeMap` in this crate, so each row's columns serialize as `ios`, `mac`, `material`, `neutral`, `terminal`. All five keys are present, and the nine rows stay in schema order. Every other object on this wire sorts the same way.

## Round 1

| Finding | At this HEAD |
|---|---|
| Missing file / refused design reported as no override | Fixed. `as_base` goes through `lookup` → `design_with`. The test refuses `none.tamber`. |
| `bases` missed package tokens, params, overridden bases | Selected folds are fixed: `bases_of` reads the checked token, so a package token, a param, and `space.xl = space.lg` are included. The unselected default above is still dropped. |
| `space.md` as a component param became the base | Fixed. `tamber_base.rs:123` keeps the param unless the looked-up name is exactly the request and a base name. |
| Quoted `"space.md"` counted as a base | Fixed, and `body.face` asserts `bases` is absent. |
| `describe` mixed two snapshots | Fixed for `describe` (compare `files`, retry). Still open for `resolve` / `explain` (the should-fix). |
| Shipped target with no column disappeared | Fixed. `steam` stays, `column: null`. |
| Terminal column advertised before the host selects it | Answered, and still accurate. `host/terminal/src/host.rs` boot does not call `set_column`; the neutral cell is what that host reads. The five columns and `column_for("terminal")` stay, which is the 16:32 map; selecting the column is B2's. |

## What holds

Cells come from `scale::row`, `cell`, `COLUMNS`, `css`, `names`, and `is_base`. Nine names in table order. Each column is `{value, source}`: px text via `scale::css`, `radius.full` is `"full"` with `{kind: "full"}`, terminal `Cells(0)` is `"0"`, other terminal cells are `"<n>cell"`, and sources are `metric.ref`, `measured.id`, `guideline.cite`, `full`, and `terminal`. `stretch` is not a `ScaleSource`, so it is not emitted. `override` is null or `{file, line, col, text}`: exact token name (the same test as `check.rs:97`), `text` is the default value, `line`/`col` are the name's (the test locks `radius.lg` at 9:3, `"20px"`), and `file` is the request's `file` or the app design (`tamber_base.rs:34`). A `when` cannot declare a token (`syntax.rs:511`). A package that declares a base name is refused in `join` before this code runs. Another `.tamber` is the design only when `file` names it.

Unset `resolve` is `{name, base, files}` with no tuples. Set `resolve` is the usual tuples plus `base`. Unset `explain` is `{kind: "base", name, base, files}`. Set `explain` keeps `token`, `value`, `whens`, `dials`, `names` and adds `kind`, `name`, and `base`.

Whole words hold because the checker uses `css::ident`, which keeps `space.md2` and `x.space.md` as one ident, and `scale::names()` is exact. `when` values that a tuple selects are in `token.values`. `light-dark(space.md, space.sm)` never reaches `bases`: an unset base inside a call is `tamber-base-expression`, and a set base is a length, so the design is refused. Duplicates keep the first.

`targets` is `shipped_targets` (the same `targets()` list `draws` uses), each with `column_for`. The host-source test checks the Apple `cfg!(target_os = "macos")` arm order, web and Linux neutral, and Android's `show_material_look()`.

The tests lock every cell's value and source against the kernel, `steam` with a null column, a quoted family, one overridden base, a missing file, and that host map. They still leave the exhaustive-`when` default, a `file` request's `override.file`, a package token, and a param default untested.
