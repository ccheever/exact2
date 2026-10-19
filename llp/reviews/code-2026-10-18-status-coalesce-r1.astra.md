# Code review: a design-only build-status told once per 150 ms, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the coalescing commit (1665bbe4a).
- **Method:** one brief (sha256 `977712ad246ee894e976321410696fe6ae521a1eab1e1ff18740a0d4ffe58ac9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: a selection latched against a design not yet told; timers outliving their registration; mode key order; the test; the doc).
- **Disposition:** fixed in round 1's commit (bce9dcd95; see its message).

---

One blocking correctness issue found in `1665bbe4a`.

1. **Blocking — [scripts/tamber-builds.mjs:171](/tmp/rv-cs-1/scripts/tamber-builds.mjs:171): selections can be associated with the wrong design.** After published `d0`, a `d1` status waits, but the server immediately accepts a selection under `d1`. `waitChanges` records that selection against its cached `d0`. When the timer publishes `d1`, the valid selection becomes permanently stale. Worse, restoring `d0` before the timer fires cancels it, leaving the `d1` selection marked current and editable indefinitely. Both reproduced. **Fix:** publish current build facts before the accepted `selection`, using existing wire events, and cover the explicit refresh path too.

2. **Should-fix — [scripts/tamber-builds.mjs:172](/tmp/rv-cs-1/scripts/tamber-builds.mjs:172): pending timers outlive their build’s connection and registration.** Closing a stream while `d1` waits announces `builds: []`; the timer subsequently emits its retained build because it checks existence, not connectivity. The tool then re-adds the disconnected build indefinitely. Replacement and eviction also leave timers allocated: 1,500 replacements produced 1,500 pending timers with one live build. Server shutdown does not cancel them. **Fix:** cancel pending work on disconnect, replacement, eviction and shutdown; reset comparison state on registration; validate the current registration and connection before firing. The two comparison maps are bounded at 513 entries, but the timer map lacks that bound.

3. **Should-fix — [scripts/tamber-builds.mjs:170](/tmp/rv-cs-1/scripts/tamber-builds.mjs:170): JSON member order defeats facts equality.** Equivalent `status.mode` objects with reversed key order produce different keys. Ten otherwise identical reports with changing designs and alternating mode-key order produced ten immediate events. Apple reconstructs its mode dictionary each report and serializes without sorted keys. **Fix:** compare mode entries independently of order, or canonicalize object keys before constructing the facts key.

4. **Should-fix — [scripts/tamber-builds.test.mjs:635](/tmp/rv-cs-1/scripts/tamber-builds.test.mjs:635): the test does not enforce the 150 ms interval.** All five reports arrive synchronously, followed by a 200 ms sleep. All seven assertions also passed with an in-memory mutation changing the interval to **1 ms**. **Fix:** use controlled time, space reports across the interval, check the boundary, and verify continued trailing delivery during sustained input.

5. **Nit — [docs/tamber-protocol.md:897](/tmp/rv-cs-1/docs/tamber-protocol.md:897): specify trailing delivery explicitly.** “At most once every 150 ms” also permits a leading throttle or a debounce that postpones delivery throughout a continuous scrub. **Fix:** state that the first pending design change schedules delivery 150 ms later, subsequent reports replace its value without restarting the timer, and an immediate status cancels that pending delivery.

The 19 in-process channel tests passed. Reproductions ran in memory; no files were edited.
