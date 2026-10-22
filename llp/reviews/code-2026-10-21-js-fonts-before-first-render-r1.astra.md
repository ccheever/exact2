# Code review: the JS target's declared faces load before its first render (the clone's wasm-vs-JS widths), round 1 of 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `f7cbecf717`.
- **Method:** one brief (sha256 `246b1e4d44479734fc1636b1e4e30021be65e1f9fc786ba7d786a11e4646d36f`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (the JS target's boot, the lead's 00:00Z task), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (3 SHOULD-FIX, 1 NIT).
- **Disposition:** taken (the commit message's "Round 1").

**Verdict: SHOULD-FIX.** The JS side is the right place to fix this, but the asynchronous gate exposes two boot-order problems.

1. **SHOULD-FIX — [build.mjs:436](host/web-js/build.mjs:436).** In the Rust `else` branch, `requestAnimationFrame(...load...)` is scheduled immediately while `go(start)` waits. With slow fonts and fast Rust loading, installation can precede the first mount. This moves data-module work ahead of the intended first frame and changes whether initial resource reads use compiled values or live answers. Schedule that rAF inside the gated callback, after `start()`. The `wait || mixed` branch correctly overlaps the two waits.

2. **SHOULD-FIX — [build.mjs:429](host/web-js/build.mjs:429).** TypeScript setup already creates a partial `globalThis.exact` ([ts-data.js:147](host/web-js/ts-data.js:147)). The new yield lets the dev client’s [readiness check](host/web-js/dev.mjs:708) resolve before `start()` supplies `state`, `clock`, and `plan`. Dev registration, design updates, or checkpoint capture can therefore run prematurely. Make dev readiness wait for completed boot. The agent drivers already wait for `data-boot-ms` before awaiting `exact.ready`, so their normal launch path is safe.

3. **SHOULD-FIX — [fonts-boot.test.mjs:99](host/web-js/fonts-boot.test.mjs:99).** The fast case unconditionally requires every load to finish before mounting, although correctly timing out after 100 ms permits the opposite. A busy CI worker can fail correct code. Control completion/deadline ordering for this assertion. The slow case’s ≥95 ms tolerance is reasonable, but its <1000 ms ceiling remains sensitive to scheduling; the fixed 300 ms sleep is unnecessary. The no-font assertion at line 128 proves no loads occurred, **not** that rendering avoided a wait. The browser dependency is appropriate: `suiteBrowser()` uses the Chromium infrastructure already required by the async glue lane.

4. **NIT — [build.mjs:427](host/web-js/build.mjs:427).** Enumerating all of `document.fonts` also fetches unrelated stylesheet/programmatic faces and can impose the full delay on an app declaring none. Restrict this to declared faces if embedded/custom documents are supported.

Production receives the same gate. With an empty font set, startup remains synchronous; rejected loads cannot strand boot. `prepareDev`/`finishDev` still bracket mounting, and capture keeps early taps queued until mounting.

For an already-painted server/boot document, this cannot recover an optional face whose fallback has already painted; that is an existing limitation, not a new adoption failure. [CSS Fonts specifies that behavior.](https://www.w3.org/TR/css-fonts-4/#font-display-desc) Wasm’s family-wide exclusion after a sibling fails also remains unmatched.

The configured 100 ms wait is consistent with wasm and a reasonable bounded tradeoff. Deduplicating preload URLs alone does not ensure every face is ready, so it cannot replace the gate.

Read-only review; no builds or tests run.
