# Code review: conform --linux: synthetic-decorations' rangeFrame held to the Linux host's specified 'unavailable' (theme2 04:27), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `984a608543`.
- **Method:** one brief (sha256 `f27a9714e7c1d299c327ca1ed3bffecf19113abfbd27ed6b8295cfc57c02825b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking or should-fix; one nit (the QUEUE recipe scrolled twice)
- **Disposition:** taken in the QUEUE commit after it

No blocking issues. No should-fix issues. One nit.

The final state matches the ruling. `synthetic-decorations` is a `LINUX_APART` entry of `['tap measure', …, { slot: 'geometry', value: 'unavailable' }]`. On that step the Linux host takes the tap, settles, reads `slots.geometry`, and fails the step when it is not `'unavailable'`; then it closes and later steps are not compared. The two-element form still stops before the step, and a `null` step still skips from the start. The comment above `LINUX_APART` states both forms. A wrong value goes through `fail`, and `--strict` prints it as `FAIL synthetic-decorations tap measure` and exits 1. The new `QUEUE.md` entry is the Linux reader, and it says the `field.rs` rects are field-local.

**Nit —** `QUEUE.md:1573`. The recipe orders a scroll after `PaintedField::place` and the field transform. `place` adds `origin`, and that origin comes from the paint rect, which `paint_rect` has already shifted by the accumulated scroll. The field transform then maps that into the viewport point `PaintedField::hit` inverts. A further scroll, done the way `frame` does it, applies that offset a second time.
DONE
