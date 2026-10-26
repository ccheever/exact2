# Code review: The web JS target writes a state-dependent -exact-layout-transition (GitHub #809), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fccf12afac`.
- **Method:** one brief (sha256 `3dd8aed4f5d2eb121687759ec1b29d08784ea068e6d36096536f37d14f58f13f`), blind to the other reviewer; 2 rounds (the standing rule for a host change), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

1. **P2 — The probe reads stale row bindings** — [rt.js:975](host/web-js/rt.js:975). `each()` and `match()` update their item/index/bound signals during `flush()`. A retained row changing from `none` to a curve therefore gets no before-layout snapshot and jumps. Reproduced: zero before style ops, one after, with the row moving. **Fix:** propagate region inputs before probing, while deferring DOM changes until after the snapshot.

2. **P2 — Ghost-parent isolation is still corrupted when the declaration changes** — [rt.js:976](host/web-js/rt.js:976). Clear a parent’s transition to `unset` while its child exits: `Paint.flush()` correctly saves empty underlying isolation, but `restyled()` makes presence overwrite that with the temporary `isolate`. It remains after the ghost disappears. **Fix:** distinguish these layout-tracking notifications from actual full-style writes, excluding them from presence’s isolation bookkeeping.

3. **P2 — Untracked probing changes `performanceNow()` semantics** — [rt.js:973](host/web-js/rt.js:973). The style effect reads committed `Now`; the probe runs with `Listener` and `Writes` null, so it advances and reads the wall clock. Crossing a conditional’s time boundary during settlement or a deferred view-transition update can miss a gained curve. Reproduced with committed time 150 and probe time 250. **Fix:** evaluate the probe using the same committed clock as the style effect.

4. **P3 — Later cleanup callbacks reinsert removed views** — [rt.js:973](host/web-js/rt.js:973). `onHeight()` and `onTRelease()` register cleanup after `lt()`. Their cleanup calls `viewId(e)`, which reinserts the element into `Views` after this deletion. Reproduced for both handlers. **Fix:** capture their IDs when registering, as `mo()` and `mh()` already do.

Verified with in-memory Bun probes using checkout code. Full Cargo/browser suites were not run: the sandbox is read-only and build artifacts are absent. No files edited.
