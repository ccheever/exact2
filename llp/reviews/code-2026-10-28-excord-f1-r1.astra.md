# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), round 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `9e4ceebb47`.
- **Method:** one brief (sha256 `3fcad3f1652b56fa60114713f9a5e527febf02aaa93e8803c246e3549a9aa35c`), blind to the other reviewer; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 1c454f3957: an aborted page was made no failure; fetchPage and leadOlder need reading; the lead needs away; Jump to Present aborted too; the 149-row claim qualified; tests through the window and the contract; the fixture test by ordering. A1 (native abort) was first answered as a runtime limit, then round 2 showed it unsafe and the abort was removed (d7da823515).

Static review of the specified diff; no edits, builds, or tests run.

1. **Blocker — Native abort does not cancel the host request.**  
   [live.ts:619](apps/excord/live.ts:619) relies on `AbortSignal`, but the native binding explicitly leaves the host request running and only rejects its JavaScript promise: [prelude.js:744](js/src/prelude.js:744), [prelude.js:767](js/src/prelude.js:767). An ordinary HTTP continuation is not one of the waiters reconsidered by [source.rs:115](js/src/source.rs:115). Consequently, the old `paged` mutation can remain pending until that HTTP request finishes, blocking the next channel’s page through [app.contract:372](apps/excord/app.contract:372). The Bun abort mock does not establish cancellation on the native platforms F1 targets.

2. **Should-fix — Leaving a channel can restart its canceled page.**  
   [app.contract:557](apps/excord/app.contract:557) calls `viewChannel("")` but retains `open`, `anchor`, and the window. Neither `fetchPage` nor the new `leadOlder` requires `reading`: [app.contract:369](apps/excord/app.contract:369). The abort becomes `{ok:false}` through [app.ts:341](apps/excord/app.ts:341), and [app.contract:427](apps/excord/app.contract:427) increments `pageTries`, rearming the same request behind the channel list. A successful retry can then continue the lead. This is bounded by the existing retry/lead limits, but defeats cancellation on leaving and counts intentional cancellation as a failure.

3. **Should-fix — Jump cancellation misses cached targets and Jump to Present.**  
   The only jump-related abort is inside `pos < 0`: [app.ts:318](apps/excord/app.ts:318). A target already cached outside the displayed window has `pos >= 0`; Jump to Present sets `anchor=""`, taking the tail branch instead: [app.contract:669](apps/excord/app.contract:669). Neither aborts the old edge request. Cancellation therefore follows “anchor missing from cache,” rather than “reader jumped.”

4. **Should-fix — The lead can fetch at the live end while `away == false`.**  
   [app.contract:613](apps/excord/app.contract:613) still assigns `anchor=win.firstId` when `isAway=false` and the first visible index is below 15. For example, a tall viewport showing 39 of the initial 49 rows can be at the bottom with index 10. After one older page lands, that index becomes 60; the new lead accepts the nonempty anchor despite `away=false` and fetches again. The `(away or anchor != "")` condition at [app.contract:369](apps/excord/app.contract:369) therefore does not establish the advertised live-end exclusion. The initial edge behavior predates F1; the automatic continuation is new.

5. **Should-fix — The unconditional 149-row bound is false with cached history.**  
   Re-anchoring uses a centered 300-row slice, which can expose 150 cached predecessors at once: [store.ts:1037](apps/excord/discord/store.ts:1037). Concrete example: with messages 1–1000 cached, a window of 701–1000 and reader at 800 gives `topAt=99`. Re-anchoring at 701 produces 551–850, leaving **249 rows** above the reader. No REST request is needed for this counterexample. The claim at [app.contract:368](apps/excord/app.contract:368) needs qualification or a corresponding behavioral constraint.

6. **Should-fix — The new tests do not prove the contract-level lead and cancellation guarantees.**  
   [live.test.ts:797](apps/excord/live.test.ts:797) manually advances anchors for six iterations. It proves page selection and retention, but would pass with `leadOlder` deleted; it never proves automatic continuation or stopping at the reader’s threshold. The abort test manually reproduces the keep predicate at [live.test.ts:840](apps/excord/live.test.ts:840), bypassing `loadWindow` and contract retry handling. It also never actually pages A again after claiming A may do so. These omissions explain why findings 1–5 are not covered.

7. **Should-fix — The fixture-delay test has wall-clock flakiness.**  
   [transcript-server.test.ts:148](apps/excord/fixture/transcript-server.test.ts:148) and [line 153](apps/excord/fixture/transcript-server.test.ts:153) require undelayed requests to finish within 250 ms. Scheduler or socket delays can fail those assertions without any fixture regression. Use controlled timer release or observable completion ordering to distinguish delayed and undelayed paths.

For the remaining checks:

- **Lead termination:** With stationary, retained `topKey` and ordinary uncached pages, I found no runaway cycle. Each landing advances the reader’s index toward 100; exhaustion also stops it. `leadOlder`, `scrolled`, and `older` explicitly exclude a pending `target`.
- **Abort ownership and cleanup:** I found no defect in preserving the destination channel or the jump’s matching around query. The identity-checked `finally` at [live.ts:630](apps/excord/live.ts:630) correctly clears its own map entry. The native pending-mutation limitation is separate.
- **RFC compliance:** No new request shape or page exceeding 50. [rest.ts:56](apps/excord/discord/rest.ts:56) enforces the limit; [live.ts:614](apps/excord/live.ts:614) rejects concurrent pages for a channel.
- **Cap and pin:** No additional 200-row assumption found that breaks this change. `windowAround` derives its center from the count. `HALF=100` still selects a tail pin that reproduces the same tail window; it need not equal half the cap.
- **Changed expectations and other code:** I found no concrete regression concealed by the changed anchor expectations. The added virtual-clock advances are not wall-clock sleeps, though they do not prove prompt continuation. The clamp test correctly checks 300. No leftover paging `LEAD` or newly introduced read-after-assign defect found; `compose.ts`’s `LEAD` is an unrelated, used regex.
