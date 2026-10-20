# Code review: LLP 1122 T1 commit 3 (the dials panel and the token grid), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `540fe0e2a` (commit `73a5d1c9a` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `f667a29dd5013e68f7dfc82285aa036ba0010627d4b426a013264acb4f3e32e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 3 low).
- **Disposition:** all taken, as in the astra record; also a length's step is read in px (and with none, fine enough for the value), its stepper given the number; a dial's box is found as its group's direct child, so a hyphenated name is drawn; a cell's accessible name adds its colour (a fallback's role and value).

---

Round 1 and 2 are fixed where they were about contrast, snapshot identity, loading and refusal, column labels, roles, and the fallback badge. One Medium remains in select options, plus three Lows.

## Medium

**A select’s options are classified and folded by a shortcut that is not the server’s colour fold.** `apps/design-tool/app.ts:142`, `apps/design-tool/app.ts:111`, `apps/design-tool/app.ts:163`

`literal` keeps a colour option only when it is absent from the token set and contains no `.`. `folded` then rewrites only hex and integer `rgb()`/`rgba()`. The checker folds a colour dial with the real colour parser (`tamber/src/fold.rs:212`, `color/src/lib.rs:132`) and stores that fold as the dial’s value (`tamber/src/ir.rs:13`).

- `ink: color = oklch(0.6 0.2 30) control=select options="oklch(0.6 0.2 30)|oklch(0.4 0.15 20)"` is a legal file. Both options contain `.`, so the panel draws a colour field and the help says those options “are not a value a dial can take (a dial is a literal).” They are literals. The same drop hits `rgb(255 0 0 / 0.5)`, which `folded` itself would turn into `#ff000080`.
- `ink: color = red control=select options=red|blue` stores `#ff0000ff`. The menu selects `#ff0000ff` and also lists `red` and `blue`. The same miss hits `transparent`, `hsl(0 100% 50%)`, `rgb(100% 0% 0%)`, and `color(srgb 1 0 0)`.
- On a number dial, `numberOf` strips a trailing `px`, so `warmth: number = 20 in 0..60 control=select options="20px|40"` shows `20px` as the current value. `set-dial` of `20px` is refused: a number dial’s value is a number.

Fix: treat an option as a literal when it parses as a colour (or, for a number, as a CSS number with no unit). Fold colour options the way the server folds a dial, compare those folds, and keep the declared spelling as the label. Drop only what does not parse, and say so.

## Low

**A length’s step is still unread, and its stepper is given the `px` spelling.** `apps/design-tool/app.ts:154`, `apps/design-tool/app.contract:509`

The range goes through `numberOf`, which accepts `Npx`. The declared step goes through `Number`, so `step=2px` (D15: a length’s step is in px) is ignored. With no step, a length gets `1` even when the value is `12.5px`, so the slider’s steps are `0` and `1` while the caption says `12.5px`. The stepper’s `value` is that same `12.5px` string, which a number input shows blank. A checked design cannot contain a length dial yet (`tamber-dial-inert`); the control is wrong on the day one is legal.

Fix: parse a length step as `0` or `Npx`, and when no step is declared use a step fine enough for the written px (so `12px` stays step `1` and `12.5px` becomes `0.1`). Give the stepper the parsed number.

**The metadata gate still does not read the control’s own range, step, or value.** `scripts/design-tool.test.mjs:157`, `apps/design-tool/app.contract:507`

The screen reader takes the bounds from the caption text (`dial-*-bounds`), the value from the caption beside the label, and the control type from the test id. The range input’s `min`, `max`, `step`, and `value` are never read. Setting that input to `max=1 step=1` leaves both the hardcoded expectation and the `panelOf` check green. The filter `/^dial-[^-]+$/` also skips a legal hyphenated dial name such as `corner-radius`, so that control is absent from `drawn`.

Fix: read `min`, `max`, `step`, and `value` off the slider, stepper, and select, and match dial test ids by prefix rather than by “no hyphen.”

**A grid cell’s accessible name still leaves out the colour it shows.** `apps/design-tool/app.ts:353`, `apps/design-tool/app.contract:399`

The round-2 name is in place: token, column label, and “fallback.” `aria-label` is the host’s accessible name (`host/web/src/element.rs:857`) and replaces the cell’s contents, so VoiceOver on `text.system`’s light cell says “text.system, Light, fallback” and never `#3c3c4399` or `-exact-secondary-label`.

Fix: append the visible colour text (`cell.note` for a residual, otherwise `cell.css`) to that label.

**A design read can still mix two revisions when a write is undone before it finishes.** `apps/design-tool/app.ts:271`

`describe` and the closing `hello` compare file digests, and tuple modes are checked. The `resolve` replies carry no digest. Describe sees digest A, another client writes B and undoes it back to A while the resolves are in flight, hello sees A again, and the grid and contrast are a mix of A and B that the digest check accepts. The next poll reloads once the file is stable, so this is a flicker, not a stuck grid.

Fix: have each `resolve` return the digests it read, and retry unless every reply shares `describe`’s digests.

Verdict: rounds 1 and 2 hold except select option folding, which still draws the wrong control for a legal literal.
