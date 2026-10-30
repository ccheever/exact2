# Code review: the exact-js overlay cut, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `66ede89624`.
- **Method:** one brief (sha256 `8dcab46b62f640eb3d71d571ec22dcfff57a6048509a22f4edb0ed8978eaa5c6`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

VERDICT: CHANGES

1. **MAJOR — [js/src/lib.rs:560](js/src/lib.rs:560): Bake initialization omits the new observable read.** `Module::inspect` evaluates the module and installs its engine without calling `__exact_has_overlay`; subsequent `load()` returns immediately. The bake uses these instances to compute answers. With the existing `overlay-odd` fixture, replaying those initialization sequences produces `reads = "0"` for inspection versus `"1"` for normal loading. Getter/Proxy initialization can therefore change baked versus runtime answers. Both entrances need the same initialization decision.

2. **MAJOR — [js/src/prelude.js:1399](js/src/prelude.js:1399): A commit-time getter still runs outside the overlay guard.** This is a remaining, pre-existing hazard. The first `exact.overlay` read precedes clearing `currentCall` and setting `overlaying`. Wasm answers retain their context across task checkpoints, so that getter can write through a captured store into the outstanding answer. An in-memory reproduction accepted the first getter’s `store.set`; only the second read was refused. Both reads need the guard.

3. **MINOR — [host/web/module-glue.js:526](host/web/module-glue.js:526): Wasm workers still encode no-overlay answers.** Worker initialization never computes or returns overlay presence, so this fallback always reports `true`. `js/web/src/lib.rs:579` consequently encodes the answer and writes before receiving `{tag:4, worker:true}`. The initialization handshake can carry the cached decision; currently workers remain an exception to the cut and LLP 1054’s unqualified load-time statement.
