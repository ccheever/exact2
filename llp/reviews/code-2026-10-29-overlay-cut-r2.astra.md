# Code review: the exact-js overlay cut, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `12f1d22a89`.
- **Method:** one brief (sha256 `89ace84681d5a306a202d0666e14765209b4968af0c66f9f79483ac9e5e83006`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Getter counts/refusals, reload values, runtime add/remove, frozen/sealed/null-prototype objects, and `globalThis.exact` accessors passed in-memory checks using the full prelude. Both Proxy traps run guarded. Wasm reply validation and its tests look correct; Rust tests weren’t run because this read-only checkout has no build artifacts. The JS target still decides at install. No files edited.

VERDICT: CHANGES

1. **[MAJOR] [js/src/prelude.js:1403](js/src/prelude.js:1403) — Proxies can falsely report no overlay.** A Proxy’s `get` trap can supply an overlay without exposing a property descriptor. Reproduced: the probe returns `""`, while the old path returns `{tag:0,value:[…]}`. Both native and wasm now silently skip that overlay. Descriptor absence must not establish absence for potentially proxied objects; add this regression test.

2. **[MAJOR] [js/src/prelude.js:1396](js/src/prelude.js:1396) — Prototype traversal can hang indefinitely.** An extensible Proxy can legally return itself from `getPrototypeOf`. The new walk never terminates; the old overlay path immediately returns `{tag:4}`. Reproduced with an external 100 ms timeout after approximately 48,000 trap calls. Bound the traversal and fall back conservatively; test cyclic and continually generated prototype chains.

3. **[MAJOR] [js/src/lib.rs:1130](js/src/lib.rs:1130) — Probe-executed module code bypasses turn accounting.** The comment’s premise is false for descriptor/prototype traps. The timer starts only at line 1179, and a negative probe exits before any budget check. A finite descriptor-trap reproduction spent 8.6 seconds in the probe versus 0.1 ms in the old path. The effects guard does not account for this work. Charge possible trap execution to the budget, including the no-overlay exit, or ensure the probe cannot execute module code.
