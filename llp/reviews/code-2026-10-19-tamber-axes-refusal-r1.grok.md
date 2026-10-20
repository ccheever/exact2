# Code review: Tamber axes bound before the fold (the JS loop's when-token edit), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `1e9ae466bf`.
- **Method:** one brief (sha256 `570b358ca2c3d575a3c6185fdf7e918f410f1a691c376821e684224fff10feab`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking, no findings
- **Disposition:** its note on the duplicate axis (a diagnostic narrower than before, from the fatal `?`) is Astra's should-fix, taken in f8ad6c5658

Nothing is blocking. No findings.

The early bind is correct for the bug it targets. A fold refusal still cannot produce a plan, and the second `AXES` write on a successful fold stays in agreement with the first.

`declare_theme` still clears `AXES` before it looks for a design. The new write at `contract/lower/src/theme.rs:233` copies the parsed `design.axes` `(name, values)` pairs only after that, and only when a design was parsed. On `check_design` failure it returns at line 396 without touching `AXES` again, so `setMode` and `-exact-mode` see the axes the file declared. On success, the assignment at line 514 runs before any action or pin is lowered. `check_design` does not rewrite `axes` (`icon::aliased` and `params::laid` leave them alone), so that later write is the same pairs.

A refused design does not build a plan. The fold error is pushed onto `l.errors` before the early return. `self.b.axis` runs only after a successful fold, and `lower_with_sites` returns `Err(l.errors)` at `contract/lower/src/lib.rs:638` before `b.finish()`. A fatal `?` is also `Err`. Unfolded axes can make a `setMode` or pin check pass, but that check never becomes a plan. Pins are safer than `setMode` here: `mode_pins` fails inside `node`, and `nodes` pushes that onto `l.errors`, so a bad pin is reported beside the fold refusal. `set_mode_args` is still a fatal `?` in `stmts.rs:117`. If the call is actually wrong, that one error still replaces `l.errors`. That shape is older than this commit. For a valid `setMode` against a refused token, which is the reported bug, the `?` no longer fires.

An axis the fold refuses is still not shipped.

- A duplicate name, a duplicate value, a host name (`scheme`, `contrast`, `color-scheme`), or too many tuples stays in the parsed `axes` vector and is bound early. `.find` uses the first copy. A `setMode` or pin that matches that first copy passes the check. The fold error is what the build returns, and there is no plan.
- A value that exists only on a later duplicate fails `set_mode_args`. The fatal `?` then hides `tamber-duplicate` and says the value is not on the first copy. That is a narrower diagnostic miss than "declares no axes" for every `setMode`. It does not ship a plan.
- A bad `under` never reaches this bind. `axis` in `tamber/src/syntax.rs` returns `None` and `parse` fails, so `design()` records the syntax error and returns `None` before line 233. `AXES` stays cleared. A `setMode` in that file still dies with "declares no axes" and drops the parse error. That path is unchanged.

Skipping pin and `setMode` checks on a refused design is not required. The build already fails, and running the checks is what stops a valid `setMode` or pin from being reported as "no axes". Skipping them would only change the residual case where `setMode` is independently wrong and its `?` still hides the fold error.

Design-server and `tamber serve` paths do not read this thread-local. `AXES` is only read by `mode_pins` and `set_mode_args`. Both run after `declare_theme` in the same lower, or after `bind_axes` in the lint. A later build with no design clears `AXES` at line 214 and returns at line 227, so it does not keep the previous design's axes. `design-serve` and `tamber_serve` use the plan, `synthetic_candidates`, and `retention`, not `AXES`. Runtime `set_mode` reads `plan.axes`, which a refused fold never fills.

The tests cover the reported failure. `a_refused_token_leaves_the_axes_set_mode_and_pins_name` requires `tamber-under-light`, forbids `tamber-mode`, and then boots a dark-side `when` edit and checks `setMode` and the pin against the window. The JS loop test checks the same refusal text in the dev log. Neither covers a duplicate or host axis, a bad `under` (still the old diagnostic), or a genuinely wrong `setMode` beside a fold refusal. Those are residuals, not holes in the fix that landed.
