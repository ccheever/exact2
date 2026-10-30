# Code review: the runner's substitute cache, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `46e65ae660`.
- **Method:** one brief (sha256 `37a3905b73380adb3427c7dd9e9a5e42ca61977637cec973bb2a98a06bf96034`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The cache key is complete for current behavior. Plan strings stay unchanged within a runner; reloads create fresh `Extensions`. Permission and member-answer mutations clone first. Malformed JSON caches the same `Null` fallback previously returned. I found no shared-mutation or threading regression.

Tests could not run: the read-only sandbox denied Cargo’s build-directory creation. No files changed.

VERDICT: CHANGES

1. **MAJOR — [substitute_tests.rs:73](runner/src/runner/modules/substitute_tests.rs:73): The required scroll regression never completes a scroll dispatch.** The fixture creates a node without handlers, so every dispatch returns `NoHandler`; this helper discards that error. Clock advancement runs, but the scroll handler and commit path do not. The uncached test also fails from warmup reads alone. Add a scroll handler that commits, assert dispatch succeeds, and measure the parse-count delta across successful dispatches.

2. **MINOR — [modules.rs:571](runner/src/runner/modules.rs:571): Distinct target strings grow the cache without a bound.** `set_module_target(&str)` accepts arbitrary strings, and old entries remain until the runner drops. Repeatedly setting distinct targets and reading substitutes retains another parsed copy per string ID each time. Switching among fixed platform names is bounded; arbitrary targets are not. Validate/canonicalize targets or bound retained target variants.
