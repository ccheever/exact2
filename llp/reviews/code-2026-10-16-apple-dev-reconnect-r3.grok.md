# Code review: the Apple dev client waiting for its program instead of stopping, round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** the whole range.
- **Method:** one brief (sha256 `90e25f829462b92c64dc66972f75d370218b4446cf0f664c4f7df88250134fd9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** as transcribed below.
- **Disposition:** applied without re-review (the last round).

---

Nothing blocking. A host that has already adopted a program cannot fetch or apply a generation for another one. One ordinary defect in the design-snapshot path.

**[P2] Resync the design snapshot when recovery keeps the running generation — `host/apple/Sources/ExactKit/PlanURL.swift:898`**

The host is on program A, generation G, with a design snapshot in flight (`syncing`) and newer batches queued in `waitingFrames`. The server announces program B (or `{rebuilt}`). `waitForProgram` (`842–851`) restarts that snapshot, which is the round-1 fix. The server then announces program A again with the same generation identity (the plan bytes did not change; the program hash is not part of the identity). `observe` (`575–578`) clears the rebuild state, then `handle` hits `currentGeneration() == revision.identity` and calls `beginResolution` (`433–434`), which cancels the snapshot and drops `waitingFrames`. No generation is applied, so nothing calls `syncDesign`. Those value-only edits are gone, and `/__dev/design` is not requested again. A direct batch while `syncing` is false still applies; this is only the in-flight snapshot and the frames queued behind it.

The three commits otherwise hold.

- **Safety.** With `adopted` set, a foreign or unnamed revision fails in `observe` (`559–560`) before `observed` moves and before any fetch. `{rebuilt}` returns at `878` before hello handling. `waitForProgram` always bumps `resolution`, so removing the old `terminal` checks on `resolveAndBoot`, fetch completion, and `failed` does not let an in-flight plan commit: `fetch` drops it, and `pump` / `commitCandidate` (`702`, `719`) still require `terminal == nil`. A waiting host that never adopted never calls `acceptProgram` (`561`), so it pins nothing and boots nothing. The envelope is checked (`659–664`) before `observe`. Hello is handled before the program check (`891–897`), so a foreign hello still registers.
- **Liveness.** A server that keeps announcing the other program on the open stream does not retry, resubscribe, or re-register. `waitForProgram` leaves the stream and nonce in place; registration runs only for a new nonce. Reconnect happens when the stream actually ends (`834`), delayed by `retrySeconds` (1s, capped at 5s), then the refusal path subscribes (`670`) instead of dropping the retry. `close` sets `closed`, so a scheduled retry does not resurrect it.
- **Waiting vs. the rest.** Design frames still go through `design()` with no `terminal` gate (`883–885`); `structure` does not start a plan fetch. `connectionStatus` stays `(rebuild the host)` while `terminal` is set (`Session.swift:252`) and flips back only after a matching announcement passes the epoch/seq check. `reload()` (`448–450`) clears that gate on purpose and re-resolves; if this process never adopted, that next envelope is the first program `acceptProgram` pins and it will boot. That is the manual “start over” path, not the automatic one.
