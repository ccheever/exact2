# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), round 2, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `e75974479f`.
- **Method:** one brief (sha256 `4a9d525e2f18ceaf143028b22a321651a1a3fb0362cdb3becc1003ac63989ed7`), blind to the other reviewer; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Both reviewers found the abort unsafe on native (the runtime drops the reply and keeps the request running): removed entirely in d7da823515, so a page in flight lands as on main and one page per channel stays on the wire; caps (app.contract over 1,500 lines) fixed; the lead starts in history after a jump or restore.

Reviewed both diffs at `e75974479f`. No files changed; no builds or tests run.

1. **Blocker — native cancellation discards rate-limit information that main consumed.**  
   [live.ts:598](apps/excord/live.ts:598) passes the signal; cancellation reports status `0` to the limiter at line 600. Native nevertheless completes the request and drops its response ([prelude.js:744](js/src/prelude.js:744), [prelude.js:1483](js/src/prelude.js:1483)). A response containing a global 429 or updated bucket limits therefore never reaches Excord’s limiter. Main processed those responses. F1 also discards useful history that main cached, requiring another request when returning.

   The body-read fix releases the reservation correctly, but [live.ts:607](apps/excord/live.ts:607) similarly discards headers **already received**, including `Retry-After`. Preserve rate-limit bookkeeping when cancelling. A1’s “no worse than main” reasoning does not hold.

2. **Blocker — `app.contract` exceeds the enforced source-size limit.**  
   The additions around [app.contract:367](apps/excord/app.contract:367) increase the file from 1,496 newline-terminated lines to 1,509. The counter at [caps.mjs:165](scripts/caps.mjs:165) counts 1,510, exceeding the 1,500-line rule. This will fail `caps`; I did not execute it.

3. **Should-fix — cancelling an `around` page permanently marks its target missing for this session.**  
   The cancellation return at [live.ts:629](apps/excord/live.ts:629) is indistinguishable from a successful page. Consequently, [app.ts:411](apps/excord/app.ts:411) adds the uncached target to `live.missed`.

   Reproduction path: jump to uncached M, leave or supersede the jump before its page lands, then jump to M again. [app.ts:286](apps/excord/app.ts:286) now substitutes the live tail instead of requesting M. Reopening the channel does not clear this record. Return a distinguishable cancellation outcome and record a miss only after an actual response. The new rig test cancels a `before` page, so it misses this case.

4. **Should-fix — cancellations still consume the failure budget through `pageAsks`.**  
   [app.contract:424](apps/excord/app.contract:424) increments `pageAsks`, while the successful cancellation path resets only `pageTries`. [app.contract:265](apps/excord/app.contract:265) declares failure after three asks regardless of their successful cancellation outcomes.

   On a narrow layout, repeatedly leave for `/` while the same page is pending, then navigate Forward to the same channel. That return uses [routing.contract:108](apps/excord/routing.contract:108), bypassing `openChannel`’s counter reset. After three cancellations, paging stops with an error until Retry. A2 therefore remains incomplete.

5. **Should-fix — the `away`-only lead has a cached-jump ordering hole.**  
   At [app.contract:371](apps/excord/app.contract:371), the lead requires `away`. A jump from the live end to a cached message outside the displayed window sets `target` and immediately scrolls ([app.contract:655](apps/excord/app.contract:655)). If that scroll reports before `landJump`, `scrolled` records the new visible key but refuses to update `away` while `target` is set ([app.contract:603](apps/excord/app.contract:603)).

   `landJump` then repeats the same positioning and clears `target`, without reconciling `away` ([app.contract:690](apps/excord/app.contract:690)). An unchanged offset need not produce another event; UIKit explicitly suppresses it ([NodeViewIOS.swift:819](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:819)). With fewer than 100 rows above that destination, the lead can remain off until the reader moves. Establish the historical reading state when landing, rather than depending on another movement event. This is a static ordering finding, not a reproduced test failure.

6. **Nit — the fixture ordering test retains an implicit two-second deadline.**  
   [transcript-server.test.ts:151](apps/excord/fixture/transcript-server.test.ts:151) starts the delayed request, then sequentially awaits two socket requests before asserting that both finished first. A scheduling pause or slow socket completion exceeding two seconds can reverse that order with correct fixture behavior. The larger delay reduces flakiness but does not remove it; controlled release would make the ordering deterministic.

For the other requested areas:

- **`reading` guard:** no additional deadlock found in supported launch restore, host navigation, split-layout, or message-link paths. They reach `openChannel`, which establishes the channel route ([app.contract:509](apps/excord/app.contract:509), [routing.contract:179](apps/excord/routing.contract:179)). Push notifications are outside the implemented RFC scope.
- **Unchanged task keys:** no independent spent-key stall found. `pending(paged)` disables the gate during a request, and a false gate clears its armed state/key ([gates.rs:48](runner/src/runner/gates.rs:48)). Findings 3 and 4 are the cancellation-related stalls.
- **Latest-branch predicate:** no additional defect found. It preserves its own `{limit: 50}` request and other channels, so repeated window reads do not repeatedly cancel the replacement page ([app.ts:318](apps/excord/app.ts:318)).
- **RFC shape, size, and concurrency:** no new request shape or page-size violation found ([app.ts:340](apps/excord/app.ts:340)). I found no separate native wire-concurrency regression: these requests use the ordered HTTP lane, which serializes execution ([executor_async.rs:479](host/apple/src/executor_async.rs:479)). The rate-limit regression is finding 1.
- **Cap implementation and comments:** the 300-row clamp and qualified cached-row comment are consistent. The new tests cover their stated narrower cases, but the admitted web scroll behavior still means the contract test does not independently prove `leadOlder`.
