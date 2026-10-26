# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), narrow re-check 1 after round 3, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `e3a0b39139`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [contract/lower/src/params.rs:585](contract/lower/src/params.rs:585): probe names still collide with icons.** `taken()` omits `design.icons`. With two number params ranged `0..1`, set `p0 = 2` with `when tone=b: d`, `p1 = 0.5`, and declare icon `exact.probe0.dial0`. If that `when` is the dial’s only reader, the batch fails with `tamber-duplicate`; retrying `p0` alone clears the reader, fails with `tamber-dial-inert`, and silently returns no range error. The final range pass skips overridden params. Include icons in the occupied-name search.

- **BLOCKER — [contract/lower/src/params.rs:229](contract/lower/src/params.rs:229): the name-index fix introduces quadratic work for literal defaults.** `shown()` now builds and clones the entire index before checking whether the message contains `param.`. Meanwhile, [components():559](contract/cli/src/tamber_params.rs:559) still calls it separately for every default, set value and options value. Thus N literal defaults build N indexes of N entries; HEAD~1 returned immediately for those literals. Route these fields through `app.shown()` and retain the wrapper’s early exit.

- **NIT — [tamber_params_scope.rs:496](contract/cli/tests/it/tamber_params_scope.rs:496): the cache test loads the wrong path.** `app()` already returns `app.contract`; appending it again produces `ENOTDIR`, yielding empty, failed `AppParams`. Use `params_of(&path)` and assert that the param loaded.

Fixes 1 and 2 otherwise look correct. I found no dependency leak in fix 4 for supported ranged kinds: role aliases and `from` remain colour-valued. No stale-index mutation path found; reload replaces `AppParams` wholesale.

Source review only; Rust tests were not run because this read-only tree has no built artifacts.
