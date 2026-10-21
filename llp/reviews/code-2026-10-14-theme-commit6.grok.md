# Code review: LLP 1121 stage 1, commit 6 (theme1's minors, no-tells, the Expose and reader ports), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `74ebaf63fe0e623734c18985a98c027fd1db67a2134e1cc78b8ef415d5b4e0e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in c0910653b: the JS bound-colour reset, the scheme rule's precedence (one rule at an id and a class), no-tells depth, the LLP text; also the element lint

---

The JS target drops the new scheme-colour declarations on bound rows, so a bound colour and a bound `color-scheme` do not do what this push says they do. Static rows and the wasm host still go through `css_text`.

[P1] A bound colour on the JS target does not unset `--exact-scheme-color` — `host/web/src/css.rs:399`

`scheme_color` appends `--exact-scheme-color:initial` only inside `css_text`. The JS target's bound `color` is one `setProperty("color", …)`: `style_row` samples that string and keeps the property name, so the new declaration is thrown away. A descendant with `color-scheme` then paints `var(--exact-scheme-color)`, which is still the root theme's `CanvasText`, not the ancestor's colour. The same colour written as a static row, or written on the wasm host (the full `css_text` is inline), does unset it, and the subtree inherits. `host/web-js/conformance/theme-reach.contract` only covers the static case, `color="#c8102e"` above a static `color-scheme="light"` card.

[P2] A bound `color-scheme` on a button, input, or textarea does not re-read `CanvasText` — `kernel/src/style/roles.rs:364`

The bound re-read is `:where([style*=color-scheme])`, specificity 0. `button, input, textarea { all: unset }` in `host/web/index.html:90` sets `color: inherit` at (0, 0, 1) and wins, so the control keeps its parent's already-computed colour. A static `color-scheme` is in `#exact-root#exact-root .cN` at (2, 1, 0) and does re-read, and the wasm host puts that `color` declaration inline, so it wins there too. The conformance fixture binds `color-scheme` on a column, which has no reset, so that case passes. A native field goes the other way: the static class colour also beats `#exact-root input[data-native] { color: var(--exact-label) }` in `host/web/neutral.css:113`, so a field that only sets `color-scheme` is painted as `CanvasText`. The page's own comment says that colour does not tint a field.

Checked and not findings:

- The iOS generation check is one integer compare of `ThemeRoles.generation`. Every row is reconfigured only when that value changes, which is the point of `GroupedListIOS.swift:535`.
- `withoutTheme` matches the lexer for a column-0 `theme`: blank lines and comments stay inside the block, and the next non-empty column-0 line ends it. Expose's theme hexes are blanked (no colour tells). It does not blank the following component body.
- No port entry name overrides a role or a reserved colour keyword. `accent` is not `AccentColor`, `fieldBg` is not `Field`, and `control`, `clear`, `slate`, `rule`, and `card` are neither roles nor keywords. `field` was the collision, and it was renamed.
- Every `theme.x` in the readers and in `packages/reading` is now an entry name, including ternaries and props (`color=(on ? "pill" : "control")`, `color=color`). `markdown-parse`'s `theme.rs` is gone, and nothing still calls it. The `#4a4a40` / `#7a7a6e` outline ternary in `apps/llp/app.contract:297` and the `#00000000` inactive borders in markdown-stress were literals before this push.
- Comments in the changed files now describe the root theme block. `exact-web`'s lib tests passed, 68/68; none of them install a `CanvasText` override. `apps/expose`, `apps/markdown-stress`, and `theme-reach.contract` lower. `apps/llp` and `apps/markdown` did not, because `@exact/reading` is not installed in this worktree. Pixel identity was not re-shot. The new Mac `bestMatch` has no test here; the call asks for the closest appearance, with the high-contrast names in the list.

VERDICT: fix (MAJOR host/web/src/css.rs:399; MINOR kernel/src/style/roles.rs:364)
