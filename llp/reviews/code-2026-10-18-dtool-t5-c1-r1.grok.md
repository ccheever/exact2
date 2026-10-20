# Code review: LLP 1122 T5 step 1, the selection join's template and style rungs, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `3d9b78582aa46605f90c9758f8a5296b18ab045c145af536826b2208926b24be`), shared by both reviewers, blind to the other review; commits ae0b9b82c4 e3d912aff6. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 blocking).
- **Disposition:** taken in `82d98133ff`: a state row written under the node takes the template rung (set-row at its place with `state`, its `row` the attribute its author wrote under the state, since the map gives a state binding no `attr`), held ones too; the value as written is read at the map's UTF-8 byte column with the attribute's name checked there, as Contract writes a plain literal (escaped strings decoded, a bare `#…` colour, an exponent, a minus, nested parentheses, a positional string after); `instances` counts call-site chains (a `button "…"`'s label is no copy); a written row carries `layout` in the selection too; a row set-row can't take (a layout row, one written otherwise, a package's file) still offers the token rung where its whole value is one token; the test's fixture is map-consistent (paint shorthand `border-radius`, layout rows marked and refused, each binding at its own attribute) with UTF-8, literal and label cases. Not taken: before the server's first file read the own-sources list is empty (T4's recorded window).

Two defects block. Both are rows `set-row` accepts that this join refuses, or addresses only by falling through to the token rung. Nothing in these two commits offers a template or style address `set-row` would reject for file, line, column, digest, or shorthand name, once the value is a form the reader accepts.

Reviewed `ae0b9b82c4` and `e3d912aff6` as they stand at `e3d912aff6`. The `layout` flag they read comes from `386762a69f`. `/Users/admin/x2team/to-dtool2.md` (the 01:24 Z note) is not on this machine. The layout sentence matches `layout()` in `contract/cli/src/tamber_rows.rs`, and the conditional-class refusal matches the round-2 note already in the repo.

## Inline `:active` on the node is refused

`rowRung` bails unless the binding has `attr`:

```232:232:scripts/tamber-inspect.mjs
  if (!b?.at?.file || !b.attr) return null;
```

A real `:active` binding never has `attr`. The map writes `attr` only from `origin.written()` (`contract/cli/src/map.rs:260-267`). `Origin::State` returns `None` from `written()` (`contract/lower/src/sites.rs:109-113`) and the state arm writes `state` and `at` only (`map.rs:280-287`). The template field `state?` at `scripts/tamber-inspect.mjs:255`, which `docs/tamber-protocol.md:1106-1113` describes as the state row written under the node, cannot occur.

The server’s own view is the case. `contract/cli/tests/it/tamber_serve_rows.rs:14-16` is:

```text
column background-color="accent.fill" …
  :active
    background-color="accent.pressed"
```

`set-row` at line 6, column 9, `row: "background-color"`, `state: "active"`, value `"#000000"` is accepted (`tamber_serve_rows.rs:43-49`). The row name has to be `background-color`. `AUTHORED` maps `press_background_color` back to that (`scripts/tamber-inspect.mjs:305`); sending the press name is the mismatch `check` refuses.

On that binding the join does this:

- Value `"accent.pressed"` (one token): `rowRung` returns null, and `editable` leaves the token rung as the default (`scripts/tamber-inspect.mjs:300`). D6’s change — template default, token beside — does not apply.
- Value `"#000000"` or a plain number: `editable` says the map gives the inline `` `:active` `` row no place (`scripts/tamber-inspect.mjs:297`).

The held path never calls `rowRung` (`scripts/tamber-inspect.mjs:338`), so a held inline `:active` gets the same answer. A style’s `:active` still goes to `stateRung` / `set-state`, which is the right operation. The test locks the refusal in: `scripts/tamber-inspect.test.mjs:88-93` expects rung `token` for the token-valued press row and “the map gives no place” for `press_color`. Before this change that second reason said `set-row` edits it. `:hover` and `:focus-visible` still have no map place; that limit is older than these commits.

## The value reader is narrower than `plain()`

```245:246:scripts/tamber-inspect.mjs
  const m = /^[\w-]+\s*=\s*(\(\s*)?(?:"([^"\\]*)"|(-?(?:\d+\.?\d*|\.\d+)))\s*(\))?(?:\s*$|\s*\/\/.*$|\s+[\w-]+\s*=.*$)/.exec((lines[b.at.line - 1] ?? '').slice(Math.max(0, b.at.col - 1)));
  if (!m || !m[1] !== !m[4]) return { rung: null, reason: `\`${b.attr}\` is written as an expression or a name in scope: edit it in the source` };
```

`plain()` accepts `Expr::Str`, `Expr::Number`, and a unary minus of a number (`contract/cli/src/tamber_rows.rs:89-94`). The regex accepts one quoted string with no backslash, or one plain decimal, with one matching parenthesis pair. A miss is reported as an expression or a name in scope. The same reader serves the template rung and the style rung. `stateRung` still has the wider third alternative (`scripts/tamber-inspect.mjs:209`) that accepts a bare `#fff`.

Paint rows, so the layout check would not be what refuses them:

- `color=#1f9d62`, `border-color=#111`, a keyframe `background-color=#1f9d6244`. The lexer emits a string (`contract/syntax/src/lexer.rs:359-381`); the grammar’s own examples are those spellings (`docs/contract-grammar.md:60-63`). `plain()` is true. The regex stops on `#`.
- `opacity=1e-2`, `opacity=2.5E-3`, `opacity=-1e300`. The lexer takes the exponent (`lexer.rs:293-310`); the grammar names `min=-1e300` (`docs/contract-grammar.md:41-48`). The regex consumes `1` or `-1` and then fails on `e`.
- A quoted string with an escape (`\"`, `\\`, `\n`, `\u{2011}`, grammar lines 52-54). `[^"\\]*` rejects it. `plain()` accepts the decoded string.
- `opacity=-(0.5)` and `color=(#fff)`. The paren primary unwraps (`contract/syntax/src/parser/expr.rs:226-229`) and the unary fold makes a number or a string, so `plain()` is true. The regex allows the parentheses only as one pair around the literal. `((0.5))` fails the same way.

Quoted `"#ffffff"`, `padding=12`, `opacity=(0.5)`, `opacity=-0.5`, and `margin="8px 4px"` match, and `set-row` takes them. An unquoted `12px` does not compile. `opacity=alpha`, `true`, and `none` are refused on both sides. A multi-line value is refused on both sides; the join still calls it an expression or a name in scope.

## What holds

For a row the reader accepts, the address matches `set-row`. The file is app-relative (`ownFile`, `scripts/tamber-inspect.mjs:216-218`). Line and column are the attribute-name span. The digest is the map source’s SHA-256, the same digest `set-row` checks. `row` is `b.attr`, the attribute as written, so a shorthand is sent whole. Sending the longhand is what `check` refuses (`tamber_rows.rs:125-140`). A missing digest or a failed read returns null and the join fails closed.

The style name is the declaration, not the compiler’s rename. The walk takes the nearest `style Name` above the attribute (`scripts/tamber-inspect.mjs:248-251`). `class:Panel__card` becomes `Panel`. `parse` does not rescope, and `set-row` looks the style up by the name in the file. Two files that both declare `style Raised` are told apart by `file`. No declaration yields null, not a guessed name.

`instances` is `sitesAt` on the element span, at least 1 (`scripts/tamber-inspect.mjs:324`, `361-366`). The new test expects 2 for two nodes at one span. The style rung has no `instances` field. A state binding is counted only when its `at` equals the element span; those spans differ (tag versus attribute name), and nothing here shows an inflated count.

An own row whose whole value is one token, and whose text the reader accepts (`background-color="surface.card"`), is now the template or style rung by default, with `token` and `modes` beside it (`scripts/tamber-inspect.mjs:289-291`). A param still wins. That is the D6 change, and it holds for those rows. It does not hold for the inline `:active` case above, or for a literal the reader rejects: that refusal returns before the token fallback, so the token is not offered beside it either.

A held style `:active` stays on the state rung. `rowRung` returns null when the origin is a style and the binding has a state (`scripts/tamber-inspect.mjs:235`), and `editable` prefers `stateRung` (`:276-279`).

A package file is refused with the package reason, by the same rule as `stateRung`: not under the app, under `node_modules`, or absent from `own` when the server passes that set. A moved file replaces the rung with the read-only reason and keeps the token as `link` (`scripts/tamber-inspect.mjs:355`). A style edit has no digest of its own; the moved-file check is what blocks it.

A layout row is refused up front with `set-row`’s reason (`scripts/tamber-inspect.mjs:237`, the same string as `tamber_rows.rs:98-99`), before a template rung exists. If the whole value is one token, that token is the rung (`editable` line 292). A param still wins. The map sets `layout` with the same `paint_row` test `check` uses, including a shorthand that expands to any layout row, and the join trusts that flag.

## Smaller points

A package paint row whose whole value is one token loses the token rung. The package reason is returned before the token fallback (`scripts/tamber-inspect.mjs:239`, then `editable` line 293), so the token is only a `link`. A package layout row still gets the token rung, because the layout return happens before `ownFile`. Before `ae0b9b82c4`, an own row of one token was the token rung even in a package file. `set-token` edits the app’s token, not the package file. The new test does not cover a package token.

`class:A/B` is refused before the param rung and the token rung (`scripts/tamber-inspect.mjs:281`). That early return predates these commits; `e3d912aff6` documents it. The style regex would not have treated it as a style anyway.

While the server’s known-file set is still empty, `own` is null and only the `node_modules` check applies. That window is the same one `stateRung` already had.

The padding fixture offers a template rung for `padding` because the synthetic map omits `layout`. On a real map that attribute is a layout row and is refused.
