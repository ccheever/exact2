# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `d7da823515`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 9cf6884bb0 (a jump's highlight runs from its landing; the jump test pins topKey through the lead) and 8b26ddea8e (openChannel clears a stale topKey; the body test holds the headers' limits).

Found **two should-fix issues and one nit; no blocker**. This was a read-only inspection; I ran no tests or builds.

1. **Should-fix — the highlight guard can expire before placement begins.**  
   [app.contract:348](apps/excord/app.contract:348) starts the 2.5-second timer when `highlight` changes, which happens when the jump is requested at [app.contract:638](apps/excord/app.contract:638). If history takes longer, the highlight is already empty when `landJump` issues `scrollIntoView` and clears `target` at [app.contract:680](apps/excord/app.contract:680). The lead can then start after 1 ms, without the intended settling interval.

   Restores also set an offset target while leaving highlight empty ([app.contract:475](apps/excord/app.contract:475), [app.contract:485](apps/excord/app.contract:485)). The new `anchor != "" and win.hasAfter` branch makes this relevant even while `away` remains false. Tie the protection to actual placement and cover restores. The changed test lands after only 400 ms, so it misses the slow-response case.

2. **Should-fix — the added jump assertion proves mounting, not remaining on screen.**  
   [app.test.contract:878](apps/excord/app.test.contract:878) uses `expect tree has`. Its implementation checks node presence and `unshown`, without checking viewport intersection or clipping ([agent-test.mjs:61](scripts/agent-test.mjs:61)). A virtualized row retained outside the visible viewport can pass. The test proves that paging occurred and the target remains mounted; add a position/visibility assertion or accompanying drive to prove the reported macOS regression stays fixed.

3. **Nit — obsolete abort commentary remains; production cancellation is clean.**  
   [live.test.ts:779](apps/excord/live.test.ts:779) still says leaving/switching aborts pages and jumping aborts the old page. Those two lines now describe removed behavior.

   The fixture’s abort listener at [live.test.ts:116](apps/excord/live.test.ts:116) and negative `signal.aborted` assertion at [live.test.ts:400](apps/excord/live.test.ts:400) also remain, but both already exist on main. No `AbortController` or `abortPages` remains under `apps/excord`. Direct comparison confirms **`live.ts` otherwise matches local `origin/main`**, with only [the body-failure handler](apps/excord/live.ts:594) differing.

4. **Clean — the lead predicate’s history/live-end distinction, apart from finding 1.**  
   At [app.contract:367](apps/excord/app.contract:367):
   - `away == false` plus a newest-reaching window (`hasAfter == false`) prevents chaining, including the short tail anchored by the 15-row rule.
   - A newest-reaching window **can** chain when `away == true`: the reader is reading back, so that is appropriate.
   - Anchored interior history can start without waiting for `away`, provided `topKey` identifies a row.
   - `target != ""` blocks the lead while a jump is owed. The remaining gap is settlement after that target clears.

5. **Clean — no `reading`-guard loading deadlock found.**  
   Launch restore and navigation resolve channel metadata through `wantChan` and `listed`, independently of history ([routing.contract:163](apps/excord/routing.contract:163)). They call `openChannel`, which establishes the channel route ([app.contract:504](apps/excord/app.contract:504)), satisfying [reading](apps/excord/app.contract:266). Host navigation/Forward, supported deep links, and split selection follow this path. Lock/photo coverage intentionally pauses further paging.

6. **Clean — limiter bookkeeping is correct; the test genuinely proves release.**  
   [live.ts:599](apps/excord/live.ts:599) releases the captured limiter’s reservation once, preserves status/headers, and rethrows. [rest.ts:217](apps/excord/discord/rest.ts:217) removes the flight and applies limits, including header-based 429 handling.

   [live.test.ts:812](apps/excord/live.test.ts:812) resolves `fetch` with a real `Response` whose body errors. Without release, the second request would be refused before `fetch`, so its error expectation and `asked == 2` would fail. **It does not test header preservation or 429 handling**: its response is 200 with no rate-limit headers.

7. **Clean — RFC 0001 and the line cap remain satisfied.**  
   [live.ts:612](apps/excord/live.ts:612) reserves the channel until the awaited request/body finishes and releases it in `finally`; navigation no longer releases it early. [app.ts:330](apps/excord/app.ts:330) retains the existing request shapes with `limit: 50`, also enforced by [rest.ts:56](apps/excord/discord/rest.ts:56). `app.contract` is **1,499 lines**. I found no other issue in the two commits.
