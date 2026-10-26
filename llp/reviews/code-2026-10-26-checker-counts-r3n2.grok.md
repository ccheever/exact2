# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), narrow re-check 2 after round 3, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `f0876099b3`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

CLEAN.

The three fixes hold, and nothing else on a design can take the reader's name.

1. The reader stem in `written_ranges` (`contract/lower/src/params.rs:588`) now treats tokens, icons, param lines, dials, and axes as taken. The name that is actually written is `exact.probe{n}.dial{k}`. `icon::check` refuses that when an icon already has it, which is why `exact.probe0.dial0` used to fail the batch, drop the readers, and leave dial `d` inert so `App.p0 = 2` was never reported. The new test places the icon on that name and still gets `tamber-param-range` for `p0`. Dials and axes are identifiers, so they cannot equal that dotted name; scanning them does not change the result. Param lines are only `Component.param` (one dot), so they can match the bare stem `exact.probe0`, which is already scanned, and they cannot match `exact.probe0.dial0`. Exports can be that dotted name, but an export is not a declaration: adding the reader satisfies it instead of failing the fold. Component params become `param.f<hash>.…` inside the probe, which cannot match `exact.probe…`. Package tokens and icons are already in the merged design's token and icon lists.

2. `params::shown` returns before `Shown::new` when the message has no `param.`. `Shown::text` still does that check, so the result is unchanged. `components()` and `param_values`'s message closure both call `app.shown`, which is the `OnceLock` over `app.tokens` — the same set those call sites used to pass in.

3. `app()` already returns the `app.contract` path, so the old `path.join("app.contract")` read a missing file and loaded no params. `params_of(&path)` plus `!failed && tokens.len() == 1` loads the one `p0` param, and `fold_with` is called with that value. The cache lives in the design crate, so this integration test still hits it.
