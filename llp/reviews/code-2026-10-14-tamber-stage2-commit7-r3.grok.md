# Code review: LLP 1121 stage 2, commit 7, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `5ef1e8b8d2173e54d0879e0ba0116bac7c67b65f8afd5116cda1b9eb88d4af59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: one high, one medium, three low.
- **Disposition:** applied without re-review: admission isolated and failing closed, implied after a clear, inline theme names; named and left: profile colours in contract tamber (a note says contract build checks them), a guide's tamber fence checked by the crate only

---

**[high]** `contract tamber check`, `eval`, and `ir` refuse profile colours the compiler accepts — `contract/cli/src/tamber.rs:74`

`theme / brand = color(--dci-p3 1 0 0)` is a standard LLP 1100 colour. The kernel parses it with no declaration (`kernel/src/style/profiled.rs:85`, and the kernel test `color(--dci-p3 1 0.5 0)`), and the lowerer lifts it out of the crate fold as `transparent` before `check_design` (`contract/lower/src/theme.rs:370`). The new verb runs `exact_tamber::check` on the raw text first and returns at line 80. `color()` there only knows CSS interpolation spaces (`tamber/src/color_fn.rs:372`), so `` `--dci-p3` is not a `color()` space `` and admission never runs. A custom `color(--press …)` declared with `@color-profile` in `app.contract` fails the same way. Admission would not save the custom case either: the stub at `tamber.rs:145` is an empty `column` with no `@color-profile`, and the copied `app.json` does not hold profiles. The comment at `tamber.rs:127` says the manifest supplies them. A design a real build accepts is rejected by the command the recipe tells you to run.

Strip `color(--…)` the way `profiles()` does before the crate check: standard profiles, plus `@color-profile` taken from the app’s real `app.contract`. Run admission against that same contract, still reporting only diagnostics whose file is the design, so `lower-theme-value` still sees the real declarations. Add a CLI test that `color(--dci-p3 1 0 0)` passes, a declared custom profile passes, and `color(--nobody …)` is refused.

**[medium]** After a boundary binding clears, `token()` reports `implied: false` — `host/web-js/modes.js:182`

On a `data-x-b` element, `unset` or `inherit` makes `Sx` return null so the implied row stands (`modes.js:145`, and `modes.test.mjs` already checks that the style becomes `var(--x-text-secondary)`). `css()` records that clear as `e.$css[prop] = null` (`host/web-js/rt.js:829`), then `apply()` writes the ancestor’s `var()` back into `$xs[j]` and the inline style. `token()` treats the row as implied only when `$css[prop] === undefined`. `null` fails that test, so the boundary rewrite is reported as not implied. A static pin, where no binding has ever set `$css`, still reports `implied: true`. A binding that sets a real value stores a string, so the pre-apply window stays correct under a looser test.

Treat a cleared binding as absent: implied when `$xs[j] != null` and `$css[prop] == null`. Cover it in `modes.test.mjs` by calling `token()` with the sixth names argument: static implied true, clear-after-apply implied true and the name resolved, an authored set implied false, a folded token absent.

**[low]** Admission fails open when the temp app cannot be written — `contract/cli/src/tamber.rs:142`

Any failure of `create_dir_all` or the three writes returns an empty vector. The caller then prints “checked” and exits 0, so `lower-theme-name` and `tamber-role-axis` are missed. Exit 2 and say which write failed.

**[low]** A `tamber` fence in the guides is still checked only by the crate — `contract/cli/tests/it/docs.rs:232`

`exact_tamber::check` does not know `lower-theme-name` or `tamber-role-axis`, so a guide example with `top = #000000` or a role that varies by an axis passes the fence. A later `contract` fence in the same section compiles against the written `app.tamber`, which is why `docs/recipes/tamber.md` is safe, and any `#` heading drops the design. Run the same admission the CLI uses on the fence.

**[low]** A relative colour over an inline `theme` name is still a tell — `scripts/no-tells.mjs:288`

`designTokens` collects names only from `app.tamber` or the file `app.json` names. `withoutTheme` (`no-tells.mjs:95`) blanks a root `theme` so its literals are not tells, and does not add those entry names to the set `relativeTo` consults. `rgb(from brand r g b)` where `brand` exists only in that block is reported, while the advice at `no-tells.mjs:327` says the root theme and the design are both the palette. `color="brand"` whole is unaffected. Collect the root `theme` entry names in `tellsIn` before the block is blanked.

## Assessment

The round-2 fixes hold. A verb’s wrong flag is refused in `fits` (`tamber.rs:52`) before the design is read, and the usage lines name each flag and allow it before the path. `top = #000000` is refused as `lower-theme-name` through admission and is tested (`tamber_cli.rs:74`). `tamber-role-axis` uses that same design-file filter. On the JS target, `Modes::js` passes each varying token’s name by custom property, and `token()` resolves `var(--x-…)`, the element’s tuple, and a static boundary rewrite. A token folded inline is left unnamed there, as intended. For no-tells, `alpha` counts as a channel keyword, and a relative origin that is an expression is exempt only when that origin has no tell of its own: `color-mix` of two tokens passes, `white` in the mix is a tell, and `rgb(from rgb(0 0 0) …)` is a tell. The start-here brand line stays a colour. Sizes are a later stage.

Nothing new on the cap. `MAX_TUPLES` is still 64 (`tamber/src/check.rs:12`), and this commit does not emit a tuple the crate refused.

The tests that would have caught the two real holes are absent: no CLI case for a profile colour or for `tamber-role-axis`, and no `token()` case. The clear test checks the CSS text only.
