# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `3ca4c6160` (commit `6b334829e` and its round-1 fixes, on commit 4).
- **Method:** one brief (sha256 `810b3af9f127bc88d53dbedc5e043d64db61e270d80396f8ad0b28d232c403cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix the false "no dials" state first (1 medium, 4 low).
- **Disposition:** all taken: until the first design answer the panel says "Reading the design…", and "no dials" only for a design read; a malformed describe is a refusal, not a throw; panels, grid, contrast and the edit sections render only for a design read; a cell's accessible name is its token, its column's label and "fallback" when it is one; a length range is read; a select's colours fold as the server folds them; the describe-derived half of the gate uses the tool's own `panelOf`, and the drawn panel is compared in order.

---

Round 1's two blockers are fixed, and the panel, grid, and contrast agree with the server for a valid colour design. One Medium remains: the first paint treats the unloaded placeholder as an empty design.

Reviewed read-only at `3ca4c6160` (the `6b334829e` panel plus its fixes). `1eab66090` is present only as the wiring those fixes sit on. Tests were not run.

## Round 1

Fixed, and the fix matches the failure:

- Contrast is taken from the snapshot stored under `design.answer` before `loadDesign` returns (`app.ts:256`). A newer read has a new answer, so the ratios move with the grid. A failed read stores nothing and the ratios clear. Overlapping reads each keep their own snapshot.
- `contrast` composites the text over an opaque background, then applies the WCAG 2 transfer (`app.ts:369`). `#00000000` on `#ffffffff` is 1. The fixture's `text.system` fallback `#3c3c4399` over white is `3.44:1 (fallback)`, which the contract test locks. A translucent background is the string `translucent background`.
- A control that does not fit the kind falls back to the kind's own, with a help sentence. `step` is kept only when it is a finite number above 0; with none, the step is fine enough for the written value (at least `0.01`). Priority is kept only for 1, 2, or 3. `select` needs options, matches `#rrggbb` to the folded `#rrggbbaa`, and prepends an unmatched value.
- A failed design shows its error and Try Again, and `advance` reads it again while `design.ok` is false (`app.contract:179`, `app.contract:312`). The "no dials" sentence is gated on `design.ok`.
- Columns are `axis=value`. Roles come from the IR `role`. The residual's "fallback" is its own badge. The stub refuses a file other than `app.tamber`. The gate's hardcoded panel expectation checks this fixture's group order, priority, range, step, and value (`scripts/design-tool.test.mjs:135`).
- The read-only-controls finding is moot: commit 4 wired the controls.

## Medium

**The unloaded design is announced as a design with no dials.** `apps/design-tool/app.contract:146`, `apps/design-tool/app.contract:319`

`design` (and `hello`) use `else empty(ok=true)`. That placeholder is the value until the first answer, and it is what the view shows while the request is in flight: `ok` is true, `answer` is 0, `groups` is empty. The sentence is `design.ok and design.groups.length == 0`, so it is true for that whole wait. A real answer starts at 1 (`app.ts:87`, `app.ts:256`), so `answer == 0` is only the placeholder. The view never checks `pending(design)` or `failed(design)`. The house pattern for this placeholder is a spinner while pending and empty, then the error, then the data (`docs/recipes/fetch-loading-error.md`).

Scenario: Ada's name is already saved, so she skips the name step. With the server up she sees "This design has no dials." until `describe` and every `resolve` return, including for `stub/dials.tamber`, which has six dials. With no server she sees that sentence until `loadHello` fails, and only then "No design server". A refresh after a successful read does not flash it, because a pending resource keeps its previous value.

The same condition sticks if `loadDesign` throws. `described.reply.ok.ir` (`app.ts:223`) and `columnsOf` assume a well-formed 200. A body without `ok.ir`, or an axis whose `values` is not an array, throws. A throw is not an `ok: false` answer, so Try Again stays hidden and `advance` does not refresh `design`. The module's own rule is that a failure is words in the answer (`app.ts:8`).

Fix: while `pending(design)` and `design.answer == 0`, show a loading state. Show "This design has no dials." only when `design.ok` and `design.answer != 0` and `groups` is empty. On `failed(design)`, show `failure(design)` and Try Again. In `loadDesign`, a missing `ok.ir` or a malformed axis should `return noDesign(...)`.

## Low

**A grid cell's accessible name is the tuple id, and it replaces the cell's contents.** `apps/design-tool/app.contract:349`

The cell is `role="button"` with `aria-label={`${t.name} ${cell.column}`}`, and `cell.column` is the id (`0-light`), not the header (`Light`). The host writes that attribute (`host/web/src/element.rs:857`), so the accessible name is "text.system 0-light". The visible "fallback" badge and the colour text are not part of it.

Scenario: VoiceOver on `text.system`'s light cell announces the tuple index and never "fallback" or `-exact-secondary-label`.

Fix: set the label from the column's label, and append "fallback" when `cell.residual`.

**A length dial's slider does not parse the range Tamber stores.** `apps/design-tool/app.ts:110`, `apps/design-tool/app.ts:125`

`strict` is `Number(...)`. The checker accepts a length range only as `0` or `Npx` (`tamber/src/check.rs:905`). `Number("12px")` and `Number("24px")` are `NaN`, so a dial `gap: length = 12px in 0px..24px` is drawn as min 0, max 1, number 0, step 1, while the caption still says `12px`. `select` can still match by string equality. Unreachable today: a length dial moves no colour, so the checker refuses it as `tamber-dial-inert` (`tamber/src/check.rs:865`). It will be wrong on the day one is legal.

Fix: parse a length the way `number()` does (`0` or a `px` dimension) before filling `min`, `max`, and `number`.

**A select matches a folded colour only when the option is `#rrggbb`.** `apps/design-tool/app.ts:120`

`same` lowercases and appends `ff` only to exactly six hex digits. `red`, `#f00`, and `rgb(255, 0, 0)` do not equal the folded `#ff0000ff`, so the current value is prepended and selected.

Scenario: `ink: color = red control=select options=red|blue` is a legal file. The menu selects `#ff0000ff` and also lists `red`.

Fix: fold each option the way the server folds a colour dial, and compare those values. Keep the declared spelling as the label.

**A refused design still shows the empty panel, grid, and contrast chrome.** `apps/design-tool/app.contract:312`

The red error and Try Again are right, and "This design has no dials." is gone. The Dials, Tokens, Contrast, Add, and undo sections are not gated on `design.ok`, so they render empty under the error.

Scenario: choosing `brand.tamber` shows "the stub has no design in brand.tamber", then a Dials heading with nothing under it and empty contrast menus.

Fix: render those sections only when `design.ok`.

**The describe-derived half of the gate does not check the control it computes.** `scripts/design-tool.test.mjs:148`

The hardcoded expectation above it does check this fixture's order, range, step, and value, and the unit test covers a bad control, a bad step, a select, column labels, and alpha. The assertion at line 149 compares sorted names only. `control` and `more` are computed with `control ?? (color ? "color" : "slider")` and `priority === "3"`, then discarded. That oracle still disagrees with `controlOf` for `token`, an unknown control, a stepper on a colour, a missing range, and a non-positive step. The on-screen check is the test id, not `min`, `max`, or `step` on the control. The canned `describe.ir`, `resolve`, and `explain` are compared to a real server for `stub/dials.tamber`.

Fix: assert `control` and `more` from `controlOf` in declaration order, or delete the unused oracle so the hardcoded expectation is the gate.

## What holds

Tuple order matches `modes_of` (`contract/cli/src/tamber_serve.rs:1062`): the first axis is slowest, and the fixture's index 2 is `dim`/`regular`. Light columns for a non-default `under scheme=dark` axis are omitted, including when several axes are under-dark, which matches the checker's light side. The IR writes that flag as `"dark"` (`tamber/src/ir.rs:69`). Residuals use `fallback` and the badge; other cells paint the wide CSS and score the `srgb` clip. Dial lists match `explain`, including a `control=none` dial the panel hides. Opaque fixture ratios are unchanged. `pair=` badges in D7 are absent on purpose: this commit's brief says declared on-pairs are proposed to the RFC owner and not invented, and the checker has no `tamber-pair` yet.

**Verdict: Round 1's blockers are fixed; fix the false "no dials" empty state before treating this commit as done.**
