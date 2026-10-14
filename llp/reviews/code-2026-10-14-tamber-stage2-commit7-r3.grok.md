# Code review: LLP 1121 stage 2, commit 7, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `5ef1e8b8d2173e54d0879e0ba0116bac7c67b65f8afd5116cda1b9eb88d4af59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: one high, one medium, three low.
- **Disposition:** applied without re-review: admission isolated and failing closed, implied after a clear, inline theme names; named and left: profile colours in contract tamber (a note says contract build checks them), a guide's tamber fence checked by the crate only

---

## Assessment

The round-2 fixes hold. A verb’s wrong flag is refused in `fits` (`tamber.rs:52`) before the design is read, and the usage lines name each flag and allow it before the path. `top = #000000` is refused as `lower-theme-name` through admission and is tested (`tamber_cli.rs:74`). `tamber-role-axis` uses that same design-file filter. On the JS target, `Modes::js` passes each varying token’s name by custom property, and `token()` resolves `var(--x-…)`, the element’s tuple, and a static boundary rewrite. A token folded inline is left unnamed there, as intended. For no-tells, `alpha` counts as a channel keyword, and a relative origin that is an expression is exempt only when that origin has no tell of its own: `color-mix` of two tokens passes, `white` in the mix is a tell, and `rgb(from rgb(0 0 0) …)` is a tell. The start-here brand line stays a colour. Sizes are a later stage.

Nothing new on the cap. `MAX_TUPLES` is still 64 (`tamber/src/check.rs:12`), and this commit does not emit a tuple the crate refused.

The tests that would have caught the two real holes are absent: no CLI case for a profile colour or for `tamber-role-axis`, and no `token()` case. The clear test checks the CSS text only.
