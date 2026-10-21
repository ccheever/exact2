# Code review: LLP 1121 stage 2, commit 6 (the web's app modes), round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `b6347d338183ffb0096f9c8792abc419ef3befe8ecc767f84a731af74b035cb7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, one low; Grok: one high, one medium (not a defect: the match includes the closing parenthesis), two low.
- **Disposition:** fixed in the round-2 commit; the RFC diff was main having moved, answered by a rebase

---

## High

**`host/web-js/src/rows.rs:369` and `host/web-js/modes.js:96`.** A boundary whose accent is a varying colour that does not name the accent never updates `--exact-accent`.

The companion map nulls every `var(--x-)` value on any `data-x-b` element. `write` then sets `--exact-accent` only when the re-specified value contains no `var(` at all, after line 92 has already removed the previous companion. `css.rs:343` skips only the exact self-reference `var(--exact-accent, AccentColor)`. The class path (`modes.rs:216`) drops the companion only when `accent-color` names an accent-naming token.

A bound `accent-color` of `brand` becomes `var(--x-brand)`. On an accent-only boundary (`data-x-b` is all bits, so nothing is re-specified) and on a pin or `color-scheme` boundary, the companion is dropped. Native `accent-color` resolves to this element's tuple. Buttons, symbols, and tokens redeclared as `var(--exact-accent)` keep the ancestor's computed accent. The same hole is the re-spec path: an inherited `accent-color:var(--x-brand)` is written again, and `--exact-accent` is not.

Fix: pass the accent-property list (`Modes::rule` already collects properties whose tuple text contains `--exact-accent`) into `modes.js`. Set the companion for a literal and for a `var()` that does not name one of those properties. In the binding map, return null only for the existing self-reference and for a `var(--x-)` whose name is in that list.

## Medium

**`host/web-js/src/modes.rs:217`.** `names_accent` treats one custom property as another when its name is a prefix.

`v.contains("var(--x-accent)")` is true for `var(--x-accent-mix)`. The uniqueness loop at `modes.rs:78` makes the same pairs on purpose (`--x-a` and `--x-a-2`, and the `a-b` / `a.b` / `a-b` case in `modes_tests.rs`). An element whose `accent-color` is the longer, non-accent token is classified as naming the accent: it is not an accent boundary, so accent-naming tokens are not redeclared for its accent, and on a pin or scheme boundary its class `--exact-accent` is removed (`modes.rs:218`). `each_varying_token_has_its_own_property` checks that the names differ, and does not check this match.

Fix: accept `var(--x-name)` only when the next character ends the name (`)`, `,`, or whitespace).

## Low

**`llp/1121-tamber.rfc.md:499` and `:621`.** This diff puts the colon spelling back into the live RFC. D5's pin example is `-exact-mode="dark-style:dim"` and "comma-separated". D7's tuple key is `axis:value`. The implementation, the deleted stage notes, and theme1's ruling use `axis=value`, and `modes.js` still splits pins on spaces and commas. A reader of D7 would write selectors that match no rule.

Fix: restore `axis=value`, the `dark-style=dim` examples, and space- or comma-separated pins.

**`host/web-js/agent.js:482`.** On an app that does not load `modes.js`, any `prefer` with a `mode` group fails before page and fold are set, including `mode: {}`. The fallback string is always non-empty. The wasm host's `check_modes(&[])` succeeds (`runner/src/runner/mode.rs` `window_with` with no pins). A real bad pin is refused on both hosts. The JS error is the reason sentence. The wasm and Linux agents wrap the `Debug` form, `mode: InvalidValue { event: "setMode", reason: "…" }` (`host/web/src/host/mode.rs:32`, `host/linux/src/agent.rs:542`). `setMode` itself matches: one axis, check before any window digit changes, and commands run only after a commit that stood.

Fix: when `exact.modes` is absent, refuse only a non-empty pin list, and use the same `mode: …` refusal text the other hosts return.
