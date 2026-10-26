# Code review: the JS target's declared faces load before its first render (the clone's wasm-vs-JS widths), round 2 of 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `18840ac626`.
- **Method:** one brief (sha256 `0ffd8cdad208cca3e952b777e0782c17b6ecb7c61cd2f2dd0561d1fbad2a8eab`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (the JS target's boot, the lead's 00:00Z task), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX, on the test).
- **Disposition:** taken ("Round 2"); the rendered-page QUEUE judged acceptable.

**Verdict: SHOULD-FIX.** No BLOCKING runtime regression found. Round 1’s test fixes remain incomplete.

1. **SHOULD-FIX — [fonts-boot.test.mjs:107](host/web-js/fonts-boot.test.mjs:107): the fast case still rejects a legitimate timeout.** The timing assertion now permits mounting after the deadline with unfinished fonts, but lines 108–109 unconditionally require Inter’s widths. A busy CI worker can therefore fail correct fallback behavior. Also, `null <= quick.mounted` is true, so an unfinished load passes the completion comparison. Control completion/deadline ordering, require a non-null completion timestamp, and apply the Inter-width assertions only when that face loaded before mounting.

2. **SHOULD-FIX — [fonts-boot.test.mjs:124](host/web-js/fonts-boot.test.mjs:124): the slow case no longer proves the timeout.** HEAD raises the ceiling from 1,000 to **3,000 ms**, while the server releases fonts after **2,000 ms**. Removing the timeout and awaiting every font would pass. Hold responses until mounting is observed, then release them; this tests ordering without a fragile upper timing bound. The `early.ready` assertion at line 126 also checks only promise shape, not that it stays pending until boot and subsequently settles.

The runtime changes otherwise address the introduced races. [build.mjs:441](host/web-js/build.mjs:441) schedules the deferred Rust load after `start()`; `wait || mixed` correctly overlaps font and data loading. The pending `exact.ready` covers deferred startup, and agent drivers wait for the boot marker before awaiting the final agent-install promise. `prepareDev`/`finishDev` still bracket mounting, and capture retains early taps until adoption. Hatches now follow the font barrier; they can still precede a slow Rust-dependent `start()`, but that independent race predates HEAD.

Production retains the gate. Empty or already-loaded app font sets start synchronously; unrelated families are excluded; rejected loads cannot strand boot. The no-font test verifies zero loads, rather than synchronous startup. `suiteBrowser()` uses Chromium already required by the async glue lane.

**Queuing the rendered-page limitation is acceptable** as an existing, separate first-paint issue. Neither server-rendered nor already-painted boot documents can recover an optional face after fallback has painted; this gate runs too late for those cases. Adoption itself remains intact. [CSS Fonts specifies the lifetime restriction.](https://www.w3.org/TR/css-fonts-4/#font-display-desc)

The JS side is the right side to fix. The 100 ms timer budget is a reasonable tradeoff consistent with wasm, though wasm’s failed-sibling family exclusion remains different. Deduplicating preload URLs alone does not guarantee every face is ready; merging variable-font faces into a range also changes undeclared-weight matching.

Read-only review with in-memory scheduling checks; no builds or browser tests run.
