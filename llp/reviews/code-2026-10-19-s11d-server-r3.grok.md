# Code review: the server half of stage 11 D (base names on the wire, tamber/1.14), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `30e22dbe4a`.
- **Method:** one brief (sha256 `db974cbc4efee11bbbbe292c9f54fe6ef4307f44e7997aaaade8106d1f2d2be5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this wire-change commit read-only: the three commit messages, the round-1 and round-2 findings, then the base-row shape against the code and the two allowed tests.Round-2 left one blocker on unset bases and a should-fix on mixed snapshots. I'll read the wire diff and the checker next to see whether those are actually closed.The round-2 bases and snapshot fixes are in the checker and the one-read server path. I'll run the two allowed tests and check the cross-branch chain case against that.Nothing blocks. The row shape, override detection, and target map match the brief, and both round-2 blockers are fixed. One should-fix: `bases` reports a base a token's own fold never reaches.

`tamber_serve_base` passed (2) and kernel `scale` passed (10).

## Should-fix

**`bases` unions every branch of every named token, not the bases this token's fold reaches.** `contract/cli/src/tamber_base.rs:81`

`bases_of` emits `Checked.bases` and every base name in `names`, then walks those tokens the same way. `names` is every token any line names. The child's `bases` is every base any of its lines folds, including a `when` this token never selects.

```text
label.gap = 4px
  when density=roomy: card.pad
card.pad = 12px
  when density=tight: space.md
```

Roomy, `label.gap` is `card.pad`, which is `12px`. Tight, `label.gap` is `4px`. No tuple folds to `space.md`. `explain` of `label.gap` still returns `bases: ["space.md"]`, because `card.pad`'s tight line is unioned in.

The same extra name appears when the base sits only on the child's unselected default (`b = space.md` with a `when` for every mode, `a = b`).

Direct references are right: a bare unset base is noted in `fold_kind` (`tamber/src/check.rs:1104`), including a `when` no tuple selects (`unselected` at `check.rs:983`) and a type's size. A set base this token itself names is in `names`. The tested chain `card.inner = card.pad` is also right, because every tuple of `card.inner` is `card.pad`.

**Fix:** record the closure while folding that token, at the tuple being folded. In `Scope::named`, when token `i` resolves token `j`: if `j` is a base, note it on `i`; also note the bases `j`'s value at that same tuple reaches. Do that for `value` and for `unselected`. `bases_of` should emit that list and stop walking each named token's other lines. Add the design above next to `gap.wide`.

If the wider closure is what "parallel to `names`" means, say so in `docs/tamber-protocol.md:440` and lock this design in the test. As written, the protocol says the names its values reach as the checker folds them.

## Nits

**Column keys sort alphabetically.** `contract/cli/src/tamber_base.rs:42`. `serde_json::Map` is a `BTreeMap` here, so each row's columns are `ios`, `mac`, `material`, `neutral`, `terminal`. All five are present. The nine rows stay in schema order. Every other object on this wire sorts the same way.

**The protocol's target list omits two `column_for` arms.** `docs/tamber-protocol.md:381` against `kernel/src/style/scale.rs:98`. `terminal` maps to `terminal`, and `mac` to `mac`. The paragraph tells a reader those targets have `column: null`.

**`resolve` swallows a bad `file`.** `contract/cli/src/tamber_base.rs:193`. `file(...).unwrap_or_default()` is dead after `lookup` (that already rejected a non-string `file`). `explain` uses `?`. Use `?` here too.

## Round 2

| Finding | At this HEAD |
|---|---|
| Unset base on a value no tuple selects dropped from `bases` | Fixed. `unselected` folds through `fold_kind`, which notes a bare `Length::Named`. `gap.wide` asserts `["space.md"]`. |
| Type size and a set-base alias chain dropped | Type size is fixed (`Value::Type`'s size, `check.rs:1107`); `body.text` asserts `["space.lg"]`. A set base named through another token is added by the walk (`space.md = 12px`, `gap = space.md`, `card.pad = gap`). That case is not in the test. The walk is also what over-reports, above. |
| `resolve` / `explain` joined two or three snapshots | Fixed. Each method is one `lookup` or `design_with`. Tuples, explain fields, `bases`, `override`, and `files` come from that fold. The describe retry is gone. |
| Quoted-family assertion passed on an error reply | Fixed. The test requires `token: "body.face"` before `bases` is absent. |
| Column keys alphabetical | Still true. Nit above. |
| Terminal column advertised before the host selects it | Still the answer, and still accurate. `host/terminal` never calls `set_column`; that process reads neutral. `column_for("terminal")` stays, which is the 16:32 map. Selecting the column is B2's. |

## What holds

Cells come from `scale::row`, `cell`, `COLUMNS`, `css`, `names`, and `is_base`. Nine names in table order. Each column is `{value, source}`: px text via `scale::css`, `radius.full` is `"full"` with `{kind: "full"}` on the four graphical columns, terminal `Cells(0)` is `"0"` (radii, and `space.xs`), other terminal cells are `"1cell"` and `"2cell"`. Sources are `metric.ref`, `measured.id`, `guideline.cite`, `full`, and `terminal`. Mac `space.xl` is still `metric` `grouped-label-inset`. `stretch` is not a `ScaleSource`, so it is not emitted. `kind` is `radius` or `length`.

`override` is null or `{file, line, col, text}`. The name test is exact (`t.name.value == name`), the same test as `check.rs:103`. `text` is the default value. `line` and `col` are the name's. `file` is the request's `file`, otherwise the app design. A `when` cannot declare a token (`syntax.rs:511`). A package that declares a base name is refused in `merge` before this code runs. Another `.tamber` is the design only when `file` names it. Describe reads the parsed file and resolve/explain read the laid design; for a base name those are the same token, because a package token and a param token cannot be one.

Unset `resolve` is `{name, base, files}` with no tuples. Set `resolve` is the usual tuples plus `base`. Unset `explain` is `{kind: "base", name, base, files}`. Set `explain` keeps `token`, `value`, `whens`, `dials`, `names` and adds `kind`, `name`, `base`, and `bases` when the line reaches one.

Whole words hold because the checker uses `css::ident` (`css.rs:183`), which keeps `space.md2` and `x.space.md` as one ident, and both `vocab.bases` and `scale::is_base` are exact. Quoted `"space.md"` never calls `named`. `light-dark(space.md, space.sm)` never reaches `bases`: an unset base inside a call is `tamber-base-expression` (`check.rs:1303`), and a set base is a length, so the design is refused. Duplicates keep the first. A component `space`'s param `md` stays the param: `lookup` rewrites the name to `param.<hash>.space.md`, and the base row is built only when that equals the request.

`targets` is `shipped_targets` (the same `targets()` list `draws` uses), each with `column_for`, null when the kernel has no column. The host-source test checks the Apple `cfg!(target_os = "macos")` arm order, web and Linux neutral, and Android's `show_material_look()`.

No request that used to succeed now fails. A non-base token, a param, an icon, and a `file` request still go through `lookup` / `design_with` and surface that path's error. A missing file and a refused design fail for a base name too. Describe snapshots and parses the design once (`design_with` at `tamber_base.rs:119`), not again for the rows.

## Tests

They lock every cell's value and source against the kernel, `radius.full`'s iOS `"full"`, `radius.lg`'s override at `9:3` `"20px"`, `steam` with a null column, unset and set resolve/explain, a quoted family on a successful explain, an unselected default, a type size, an unset chain (`card.inner`), an overridden base naming another, a missing file, and the Apple branch order.

They still leave untested: a `file` request's `override.file`, a package token, a param default, a set base reached through another token (`gap = space.md` with `space.md = 12px`), `space.md2` / `x.space.md`, and the cross-branch case above. `cell()` in the test repeats `value()`'s match, so a terminal spelling of `"0cell"` or `"1ch"` is caught only if the helper and the server drift apart. A literal `"0"` / `"1cell"` / `"2cell"` on `space.xs`, `space.md`, and `space.lg` would pin the wire spelling on its own.
