# Code review: LLP 1121 stage 2, commit 7 (CLI verbs, layout's tokens, no-tells, docs), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `359165893a7ddc97582e0cd25771e2836deb0a9a4a423034d04c3a7df781360c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: five medium, three low; Grok: three medium, two low.
- **Disposition:** all fixed in the round-1 commit

---

## Medium

**Medium — `layout` invents token names, and drops the one the row read — `runner/src/runner/mode.rs:200`**

`row_token` rebuilds `names` from `name_bit` (a 64-bit hash; `kernel/src/style/theme.rs` says two names may share a bit) plus `text.contains(name)`. That lists a token the row did not read when the unread name shares the bit and is a substring of the text. `brand` and `brand.label` both hash to bit 22, so a row whose text is `brand.label` is reported as `["brand","brand.label"]` if both tokens exist. The same path drops a name the row did read: a role override whose token name differs from the spelling in the row (`secondary-label` in the design, `color="-exact-secondary-label"` on the node) stores `name_bit("-exact-secondary-label")`, which is not the token's bit, so `names` is empty. `layout_of_a_node_names_the_token_each_row_read` only checks `page` and `ink`, which neither collide nor contain each other.

Record the table keys the parse actually matched when the row is indexed, and print those. Do not recover names from the refresh bitset.

**Medium — `implied: true` means "the author did not write this row", not "a boundary wrote it" — `runner/src/runner/mode.rs:213`**

`implied` is `!row.written`. Mode boundaries do set `written: false` (`runner/src/instance/theme.rs:303`), but so does every compiler-added binding. A `link` gets `color` `LinkText` with `written: false` (`contract/lower/src/tags.rs:1358`). If the design overrides `LinkText`, that row is in the index and `layout` reports `implied: true`. `scripts/agent-inspect.mjs:306` then prints "implied by the boundary".

Store a flag set only in `boundary_rows`, and have `row_token` return that.

**Medium — a relative colour with absolute channels is no longer a tell — `scripts/no-tells.mjs:163`**

Any `rgb()` / `oklch()` / `color()` whose `from` origin is a token or matches `ROLE` is exempt, and the channel values are not checked. The folder accepts numbers there (`tamber/src/color_fn.rs:111` and `:498`), so `color="rgb(from surface.card 255 0 0)"` and `oklch(from CanvasText 0.7 0.2 30)` fold to a literal and `tellsIn` returns nothing. The forms the commit names still pass: `rgb(from surface.card r g b / 0.5)` and `oklch(from CanvasText l c h / 0.4)`. Hex, named colours, and `rgb(0 0 0 / 0.5)` are still caught.

Exempt a relative colour only when each channel is that function's channel keyword (`r`/`g`/`b`, `l`/`c`/`h`, …). A number or percentage in a channel slot stays a tell. An alpha after `/` may be a number.

## Low

**Low — the published usage puts the flags where the CLI will not take them — `contract/cli/src/main.rs:31`**

`contract tamber eval --mode dark-style=dim .` and `contract tamber ir -o out.json .` take the second argument as the design path (`contract/cli/src/tamber.rs:25`). Those commands exit 2. The working order is path, then flags, which is what `contract tamber --help` and `docs/recipes/tamber.md` say. The same reversed synopsis is in `scripts/help.mjs:255` and `:276`, which is what an app's `exact tamber --help` prints. `scripts/exact.mjs:887` also sends `tamber --help` through a compiler build; the `contract` verb above it prints help without compiling.

Put the path before `--mode` and `-o` in those usage lines, and short-circuit `tamber --help` the way `contract` does.

**Low — `designTokens` only sees theme entries indented by exactly two spaces — `scripts/no-tells.mjs:259`**

The `.tamber` grammar accepts whatever indent the block's entries share (`tamber/src/syntax.rs`, "this block's tokens share one indent"). A theme indented four spaces still passes `tamber check`, and `designTokens` returns an empty set, so `rgb(from surface.card r g b / 0.5)` is reported as a literal. `when` lines are safe only because they are indented deeper than two spaces.

Take the theme block's entry indent (the first `name =` line), not a hardcoded two spaces, and ignore deeper lines.

## Caps and what was not found

No high-severity defect. `docs/recipes/tamber.md` is under 3 KB. Nothing in this commit crosses 1,500 lines.

Exit codes match the other `contract` verbs (2 for usage and I/O, 1 for a refused design). `quote` escapes `names`, `text`, and `modes`. `design()` for an app directory or `app.contract` calls the same `design_path` as the compiler. `tamber fmt`, `gen`, `export dtcg`, and the TypeScript `Token` union are absent, which matches the stage-3 deferral. The start-here line matches RFC §6.
