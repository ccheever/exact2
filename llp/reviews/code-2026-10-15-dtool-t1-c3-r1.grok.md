# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6b334829e`.
- **Method:** one brief (sha256 `15658f15d66173394479df3451f8eaa8e8b5a167a2754788b1c01eb810c54a25`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** do not land (2 high, 2 medium, 2 low).
- **Disposition:** taken, on top of commit 4 (1eab66090), which had already wired the controls (the read-only finding is moot there): each design read is its own snapshot, by number, and contrast and a cell's branch name the read they are of; contrast is text composited over an opaque background, a translucent background said so; metadata that does not fit a dial's kind falls back to its own control, said in its help, a step must be a positive number (none: one fine enough for the value), a priority 1, 2 or 3; a select's value matched by meaning; columns labelled `axis=value`; roles from the IR's `role`; a residual's "fallback" its own badge; a failed design read has Try Again and is read again after a failed poll; the gate compares every control's range, step, value and order; the stub refuses a file it has no design for. A refused design no longer also says it has no dials.

---

The dial and column mapping for a valid design matches the server. Contrast does not: it is computed from a module-level cache that the resources do not share, and the ratio treats a translucent clip as opaque.

## High

**Contrast stays on the previous design after a refresh or a file change** — `apps/design-tool/app.ts:145`, `apps/design-tool/app.ts:160`, `apps/design-tool/app.contract:113`

`stored` and `columns` are module globals. `loadContrast` reads them and does not resolve anything itself. Its arguments are `pickA`, `pickB`, `since`, and `epoch`. `file` is only an argument of `loadDesign`.

`loadDesign` writes `columns` as soon as `describe` returns, then awaits every `resolve`, and only then replaces `stored`. A failed resolve returns the error and leaves the new `columns` paired with the previous `stored`. The runner asks every stale resource in one pass. `loadDesign` is waiting on the network at that moment, so `loadContrast` runs against the previous cache and that answer is stored for the new `since`/`epoch`. When the new grid arrives, contrast's arguments are unchanged, so it is not asked again.

Scenario: `text.primary` against `surface.page` shows `18.51:1` on Light. Another client sets `surface.page`'s light side to `#000000`. The long poll advances `since` and `epoch`. The grid updates to black. The contrast row still says `18.51:1` until a pick changes. Switching the file picker does the same with no poll at all: the grid is the new file and the ratios stay the previous file's. A pick made while the new `describe` has landed and the resolves have not mixes the new column list with the old swatches.

Fix: drop the globals. Give `loadContrast` `file` as well as the picks, and resolve those two tokens inside that call. On a resolve failure, leave the previous cache untouched by not having one.

## High

**A translucent fallback is scored as an opaque colour** — `apps/design-tool/app.ts:230`

`contrast` parses `#rrggbbaa` and runs the WCAG 2 transfer on the RGB bytes only (`0.04045`, `/12.92`, exponent `2.4`, `0.2126/0.7152/0.0722`). The alpha byte is discarded. The opaque pairs in the fixture match that math: `#0f1419` on `#ffffff` is `18.51:1`, `#e7e9ea` on `#000000` is `17.24:1`, `#e7e9ea` on `#15202b` is `13.54:1`.

`text.system` is the fixture's residual. Its fallbacks are `#3c3c4399` and `#ebebf599` (alpha `0x99`, 60%). Against `surface.page` on Light the function reports `10.94:1`. The colour actually painted over that page is about `#8a8a8e`, at `3.44:1`. The row would read as AAA while the painted pair fails AA. The same overstatement is `17.73` against `6.36` on Dark, and `13.93` against `5.84` on Dim. The "(fallback)" suffix marks the source and still prints the opaque ratio.

Fix: composite a translucent side over the opaque side, then take the ratio of that composite against the opaque colour. When both sides have alpha, name the backdrop the composite used. Keep the current formula when both alphas are `ff`.

## Medium

**A control illegal for the dial's kind is still drawn** — `apps/design-tool/app.ts:98`

`none`, an unknown name, `token`, `color` on a non-colour, and `slider` without a range fall back to the kind's control. `stepper` and `select` are accepted for every kind. `step` is kept whenever `parseFloat` is finite, including `0` and negative. The checker does not validate `control`, `step`, or `options`.

Scenario: `accent: color = #1083fe control=stepper` is a legal file. The panel shows a number field with value `#1083feff`, min `0`, max `1`, step `1`, and no colour well. `lift` with `step=0` reaches the range input as step `0`. `control=select` with no `options` is an empty menu.

Fix: allow `color` and `select` for a colour, and `slider`, `stepper`, and `select` for a number. Require a non-empty `options` list for `select`. Use the kind's control otherwise. Use a step only when it is finite and greater than zero.

## Medium

**The gate does not prove that every control is what `describe` declares** — `scripts/design-tool.test.mjs:129`, `apps/design-tool/stub/tamber-stub.ts:42`

The server check sorts both sides by name before comparing, so group order and priority order can reverse and the assertion still passes. Its oracle is `control ?? (color ? "color" : "slider")` and `priority === "3"`. That agrees with this fixture and diverges from `controlOf` for `token`, an unknown control, `stepper` on a colour, a non-positive step, and a priority that only matches after rounding. The fixture has no `select`, no colour dial without `control`, and no `control=token`. The contract contrast test covers three opaque pairs and never a residual, so the `10.94:1` figure above is untested. The stub's `describe` and `resolve` ignore `file` while `hello` advertises `app.tamber` and `brand.tamber`, so the new file picker is on screen in the contract tests and cannot change the design. The live comparison never sends `file`.

Fix: compare the panel to `controlOf`'s real rules in declaration order, add one illegal control and one translucent pair to the fixture, and make the stub answer `describe`/`resolve` for the file the request names.

## Low

**A refused design is also described as having no dials** — `apps/design-tool/app.contract:188`

`noDesign` returns `ok: false` and empty groups. The view prints `design.error` and then, because the group list is empty, "This design has no dials." A server refusal and an empty dial block look the same under that heading.

Fix: when `design.ok` is false, show the error and skip the dial, token, and contrast sections.

## Low

**The dial fields are editable while this commit cannot write them** — `apps/design-tool/app.contract:261`

The colour field, range input, stepper, and select have no `readonly` and no handler. The commit message says they stay read-only until the next commit writes. A drag of Lift moves the thumb and leaves the design unchanged.

Fix: set `readonly` on each control until a write is wired.

Tuple order matches `modes_of` and the checker's digits (first axis slowest; the fixture's index 2 is `dim`/`regular`). Dropping the light column of a non-default `under scheme=dark` axis matches the rule that the axis varies the dark side only. `srgb` on a folded colour is the 8-bit clip, and a residual correctly uses `fallback` and the "fallback" label.
