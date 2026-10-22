# Code review: The web JS target writes a state-dependent -exact-layout-transition (GitHub #809), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fdb3caa8b4`.
- **Method:** one brief, blind to the other reviewer; 2 rounds (the standing rule for a host change), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Four P2 findings:

- **P2 — Blank values retain the wrong paint flags.** [paint.rs:200](host/web-js/src/paint.rs:200) uses table membership, so a named `""` gets flags `260`. The runner instead clears blanks through `bridge::unsets`, leaving no layout-transition mask bit. This changes stacking/isolation. `"unset"` also produces an incorrect invalid-value journal entry. **Fix:** handle the runner’s silent-clear values before parsing/mapping. Test through the runner; the new paint test bypasses this path by calling `set_dynamic` directly.

- **P2 — Synthetic restyles corrupt exit-ghost isolation restoration.** [rt.js:986](host/web-js/rt.js:986) puts every bound view into presence’s `restyled` set. If a ghost’s parent loses its bound layout row, `Paint.flush()` records the correct underlying isolation while retaining the temporary `isolate`. `Pres.after()` then overwrites that saved value with the temporary one. Isolation remains after the ghost finishes. Reproduced with the actual presence and paint modules. **Fix:** distinguish layout-row notifications from full style replacements, preserving `$ghostIsolation.value`; extend the ghost test with a bound parent.

- **P2 — Removed bound views remain strongly referenced.** [rt.js:973](host/web-js/rt.js:973) removes elements from `Bound`, but `bound()` registers them in the strong `Views` map through `viewId()`. Ordinary region teardown never removes that registration. A lifecycle probe left `Views.size === 1000` after 1,000 mount/unmount cycles, with `Bound` empty. **Fix:** unregister scoped views when their owner ends, and test repeated removal.

- **P2 — Disabled motion still measures every bound view on every commit.** [rt.js:207](host/web-js/rt.js:207) supplies all bound views as potential gains, including unchanged `"none"` values. An unrelated commit therefore performs synchronous geometry reads across the entire set. With 1,000 inactive rows, the counter probe recorded 1,001 bounding-rect reads and 2,001 computed-style reads; the equivalent wasm batch performed none. **Fix:** supply only bindings that may change, letting presence’s existing tracking cover active transitions.

No escaping defect found. Caps passed with wasm compilation disabled. Full Rust/browser suites were unavailable in this read-only checkout without build artifacts or dependencies. No files changed.
