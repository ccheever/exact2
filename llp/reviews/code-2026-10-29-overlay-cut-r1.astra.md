# Code review: the exact-js overlay cut, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `0dd528d127`.
- **Method:** one brief (sha256 `3c1ceb6d3c692a806892749f7153eda41a7351dfedbe2b8eb9f3f6cda1039316`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

In-memory Bun probes reproduced the getter and wasm error regressions below. Ordinary absent/add/remove/throw cases, the worker fallback, and all 18 effect refusals in the existing overlay fixture passed those probes. Rust/Hermes tests were not run: this read-only checkout has no built test artifacts. No files changed.

VERDICT: CHANGES

1. **MAJOR — The probe adds an unguarded accessor invocation.** [js/src/prelude.js:1393](js/src/prelude.js:1393)  
   An overlay getter now runs three times instead of twice. Its second invocation moves from the guarded overlay body to the unguarded `typeof` check. With an answer pending, I reproduced a getter whose second access calls `store.set`: the old path refused it; HEAD sent the write through the host door. A getter throwing on its third access also changes a successful overlay into a failure. Preserve accessor behavior without executing an additional getter outside the overlay guard.

2. **MINOR — Presence probing bypasses turn accounting.** [js/src/lib.rs:1137](js/src/lib.rs:1137)  
   The engine call precedes `TurnClock::start`, and a negative result returns before any budget judgment. A costly getter returning `undefined` therefore escapes the budget and hitch reporting that previously covered its lookup inside `__exact_overlay`. Getters returning functions also spend unaccounted time in the probe. Keep application execution within the existing turn accounting.

3. **MINOR — Wasm silently converts probe errors into absence.** [js/web/src/lib.rs:561](js/web/src/lib.rs:561)  
   The web import catches a throwing getter and returns `{error: "…"}`. Because that object has no `has: true`, HEAD returns `Ok(None)`. Previously the overlay response produced `DataError::Unavailable`, which the runner journaled; native still reports the probe error. Only an explicit successful negative response should mean “no overlay”; propagate error and malformed responses.

4. **MINOR — The required reload and web parity coverage is missing.** [js/tests/it/overlay.rs:145](js/tests/it/overlay.rs:145)  
   The added tests mutate one native module and inspect counters. They exercise neither a generation reload nor either web target. The JS target’s presence decision is made at installation in [ts-data.js:459](host/web-js/ts-data.js:459), so late addition/removal is not equivalent to the new native behavior. Add the promised reload and parity cases, including worker fallback and probe failures; assert an actual overlaid value as well as encoding counts.
