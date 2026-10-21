I'll review the proposed LLP 1041 §8.4 amendment against the current code, starting with the draft, the design note, and the admission path the diagnosis depends on.The draft's diagnosis hinges on a few dispatch and admission sites. I'll read those next, along with the repro and the fence that holds work behind a refusal.The diagnosis is right, and Charlie's (a) is the right fix. Ship (a) together with a narrow form of (b): a re-ask is a new `Dispatch::Again` that takes its place in the ordered settlement sequence already complete, and it never becomes executor work. Keep the sixteen as a cap on effects until the UI drains them. Do not free a completed write's slot at `complete`.

I did not execute the ignored repro (the test command was not run in this review). The first-wave index below is from the code; "rows 15 and 16, every run" is the draft's measurement.

## What to build

1. **`Dispatch::Again`.** `Module::ask_again` (`js/src/turns.rs:10-18`) returns it and carries no closure. A re-ask is `Request::continuation` (`runner/src/request.rs:387-392`), same shape as a storage step, so the executor cannot discover it by method. `safe` (`host/apple/src/executor_core.rs:792-799`) is false whenever `continuation` is set.

2. **Admit `Again` under the existing read backlog.** One ordered FIFO. An `Again` is appended to `ordered` with an empty success already in `completed`, charged `retained` bytes (`executor_core.rs:1002-1028`) plus `size_of::<Completed>()` against the 512 MiB lane, and one ticket against the 128 (`ORDERED_READS`). It is not a `Job`, so the single worker never runs it and it is not charged the 64 MiB response ceiling (`reservation`, `executor_core.rs:810-866`; `next_job` adds that ceiling at start, `657-684`). Behind the fence (`executor_core.rs:290-322`) it is `Fenced::Held` like any ordered request and pre-completed, in place, when `resume_ordered` lifts the fence.

3. **Two thresholds, one idle bit.**
   - **Sixteen** refuses a write or other effect: queued, running, and completed-but-undrained POST/storage/native/`Work::Later`, and queued or running GET/HEAD. A finished GET leaves this count at `complete` (that is the part of (b) that matches "a slot is held while HTTP runs"). A finished write stays until `drain` (`executor_core.rs:451`).
   - **128 tickets and 64 MiB of waiting request buffers** refuse the one read or `Again` that does not fit. The fence of `2eca0ad80` then holds later ordered work, unchanged.
   - **`ordered_idle`** (`executor_core.rs:537-538`) stays false until every ordered ticket has drained, `Again` included. A refusal settles only then (`abi.rs:324-336`), so it cannot jump ahead of an undrained re-ask.

4. **Past 128, refuse that one request and keep the answer pending.** `resume` currently drops the parked call and then returns (`js/src/lib.rs:896-903`). Put the call back, do not `release_failed` (`runner/src/runner/admission.rs:68-107`), and admit that `Again` at the front of the lift so the fence cannot fill the window with newer work first. `forget_calls` (`js/src/turns.rs:53-70`) only when the runner actually drops the ticket.

5. **Hosts.** Apple, Linux, and render share `executor_core.rs` (`host/linux/src/executor.rs:8`, `host/render/src/executor.rs:8`) and each match `Dispatch` (`abi.rs:300-308`, `presenter.rs:802-812`, `host/render/src/lib.rs:851-865`). Web settles `Again` on the immediate path (`host/web/src/host.rs:1103`). The terminal host has no admission cap and still has to answer the new variant (`host/terminal/src/requests.rs:80-97`) with the same empty success it gets from today's no-op. The JS target never sees `Dispatch`. Every other exhaustive match (data sources, tests) needs an arm.

Leave storage ops, POST, auth, surfaces, streams (their own 16, `executor_core.rs:25`), independent HTTP (128 / 32 MiB), the 48-worker cap, and D4's forget/abort rules as they are.

## Diagnosis

Nothing deadlocks. A waiting answer holds no executor slot, and the re-ask does.

Boot begins every resource in one commit, then `emit` dispatches (`abi.rs:253-284`). Each `begin` bumps `progress` before the module runs (`js/src/lib.rs:774`). The first row starts `/prefs` and parks on that ticket. Each later row awaits the same promise and is parsed as `WAITING` (`js/src/lib.rs:687-692`, constant at `152`), then parked (`994-1006`). `dispatch` sees `WAITING` and calls `wake` (`js/src/source.rs:53-61`). `wake` re-asks when `parked.progress < progress` (`js/src/turns.rs:32-41`). After twenty begins, `progress` is 20, so rows 1 through 18 re-ask and row 19 is `Dispatch::Held` (no movement yet, and `outstanding` is true because the fetch is parked, `turns.rs:23-27`). That is one fetch and eighteen re-asks.

The fetch is a plain GET: `work.is_none() && safe(...)` admits it against 128 (`executor_core.rs:333-344`) and still does `counts[0] += 1` (`375`). Each re-ask is `Work::Now` of an empty 200, so it uses `COUNTS[0] == 16`. Fifteen re-asks fill `counts` to 16; the sixteenth (row 16) is refused and `ordered_barrier` goes up (`362-368`). Rows 17 and 18 are held in `fenced` (`290-315`), not refused with it. The held map on the host (`abi.rs:302-305`; Linux `presenter.rs:804-807`; render `lib.rs:855-858`) is outside the executor. The draft's line citations match this path.

The shared load is not starved. `/labelers` is issued from the first row's resume while the barrier is up, so it sits in `fenced` until `ordered_idle` and the refusal has been taken (`abi.rs:244-248`, `324-336`). `counts[0]` falls only in `drain` (`451`), not in `complete` (`687-728`), so the lane stays busy until those no-ops drain. When a non-waiting delivery lands, `progress` bumps (`lib.rs:905`) and `release` asks every waiter (`source.rs:69-82`). Another opaque re-ask is then refused. The draft's "row 15 on the second wave" is that release; the code supports a second permanent refusal, and the repro is what pins the index.

The refusal is permanent. `take_request_refusal` marks the ticket refused (`admission.rs:49-57`). `resume` has already removed the parked entry, sees `Failed`, and returns `DataError::Unavailable` without `__exact_settle` and without `forget_calls` (`lib.rs:896-903`). `fulfill` then `release_failed`: "it keeps its last value", and nothing asks again (`admission.rs:84-107`, called from `runner/src/runner/commit.rs:1248-1257`). D2's never-restart rule (`llp/1041-graceful-overload.rfc.md:82-85` and §8.4 at `474-477`) is what makes that terminal.

The refused call's JavaScript keeps running, and this is worse than a wasted fetch. The prelude call stays in `calls`. When the shared promise resolves inside whoever `__exact_fulfill` set as `currentCall` (`js/src/prelude.js:1484-1490`), that call's `fetch(row/i)` is pushed onto the live answer (`860`). `settle`'s unclaimed scan (`1247-1250`) can then attach it to a different answer. The draft's "22 fetches, reply goes nowhere" is the mild observation; the seam can also deliver the orphan body to a live row.

Render fails differently. It never calls `release_failed`; a refused ticket is marked `busy` and the page ends `Settled::Busy` (`host/render/src/lib.rs:806-807`, `840-865`). `a_burst_past_the_limit_refuses_one_and_runs_the_rest` (`1316-1334`) is real opaque work (`continuation` returns `Outcome::Storage`), so it must keep refusing the seventeenth. Web has no sixteen: `Work::Now` runs in `immediate` (`host.rs:1103`), and the JS target's await is just an await. That is why the clone loads on the web.

## Semantics that are sound, and the holes

**Read vs effect.** The draft's read rule matches the code: GET/HEAD, no continuation, no storage, no surface, no auth, no native, no `Work`. POST stays an effect. A storage step stays an effect: the continuation closure drains whatever op is at the queue head (`data/src/storage.rs:8-9` only builds the bytes; the host runs an opaque `Work::Now`). Module turns, native calls, and background rounds (`js/src/background.rs:70-76`, `Request::continuation(BACKGROUND)`) stay effects. Background refusal already settles on its own ticket and then stalls (`background.rs:86-117`); `parse_for` clears `stalled` on the next resume (`source.rs:318-328`), so a refusal during a herd retries rather than sticking forever.

**Order.** Pre-completing `Again` preserves settlement order: `drain` only takes the front of `ordered` (`executor_core.rs:422-447`). It also matches today's execution order for real effects. The no-op closure does not run the answer's JavaScript; `resume` does, later, on the runner thread, with the store installed (`lib.rs:908-944`). A later POST can already be sent by the worker before that JavaScript runs, because the worker runs the no-op and moves on before the pump drains it. Skipping the empty closure does not move the POST earlier relative to other real jobs.

**The hole in draft (b)'s second half.** "Release every job's sixteen at `complete`" makes `counts[0] == 0` while completed writes are still undrained. `ordered_idle` would then let a refusal settle, and `admit` would run another POST, while up to 128 small writes had already been sent. D2's bound on irreversible effects ahead of the UI is the sixteen until drain (`D2` at `llp/1041-graceful-overload.rfc.md:72-85`). Bytes do not replace it: a small POST is nowhere near 512 MiB. Finished GETs are the case that should drop out of the sixteen at `complete`; they stay in the 128 and in the byte budget until `drain`. `admission_counts_completed_results_until_consumed_and_preserves_control_space` (`host/apple/src/executor_tests.rs:204-281`) covers the independent lane only. The ordered lane needs its own assertion that sixteen undrained POSTs still refuse the seventeenth.

**Fence.** Unchanged, and it must stay keyed off settlement-idle, not off the effect sixteen. If `Again` is omitted from `ordered_idle`, a write refusal jumps ahead of re-asks that were issued before it.

**Q7's "stay parked past 128"** is an uncapped side queue. Those `RequestOut`s live in the host `parked` map, which `Core::forget` does not scan (`executor_core.rs:470-524` forgets jobs, completed, running, and `fenced` only). They also settle after work that was admitted later, when a slot frees. Past the cap, refuse, and keep the one refused `Again` at the front of the fence.

**Cancellation and teardown.** An `Again` already in `completed` is dropped by `forget` when the runner no longer holds the ticket (`492-503`) and cleared with the other completed outcomes in `retire` (`599-616`). A fenced `Again` is destroyed by the worker, as queued work is (`875-904`). A host-parked `Held` is a pre-existing gap; do not add another parked state on that map. Retirement still must not run closure destructors on the UI thread. `Again` has no closure, so it is the easy case.

**D2 accounting.** Completed-but-unconsumed work stays counted: effects in the sixteen until `drain`, reads and `Again` in the 128 and in retained bytes until `drain`. Queued reads stay charged their request buffers against 64 MiB (`349-356`). An `Again` must not go through `reservation`'s response ceiling or 128 of them will stall the 512 MiB lane the way F2 did.

**Web, terminal, render.** Behavior on web and web-js stays "settle immediately" / "no executor." Terminal must compile and reply with the empty success. Render's `lift` (`lib.rs:836-848`) keeps marking a real refusal `busy`; `Again` should stop a shared-load render from ending `Busy`.

**Scope gap the draft already names.** `hiddenFirst` awaiting storage is not in this checkout. Twenty storage steps are opaque continuations and still hit the sixteen. This amendment fixes the shared-promise re-ask the repro builds (`js/tests/fixtures/shared-load.ts`). A green Home drive is a separate check if each row also awaits storage.

## Q1–Q8

**Q1 — POST reads.** No `exactRead` flag. A misdeclared POST would be queued and sent past the sixteen, which is a user write D2 says to refuse. Bluesky queries are GET. A later amendment can mark a specific XRPC procedure at the seam; an app-supplied bit is the wrong layer.

**Q2 — Storage reads.** Not in this amendment. Classifying a step read-only means the data seam stamps the op name (`get` / `list` versus `put` / `delete`) onto the request. The continuation is opaque and the queue behind it can contain a write. The repro does not include `hiddenFirst`.

**Q3 — What the sixteen counts.** Effects until drained, plus reads that are still queued or running. `Again` counts toward 128 and toward `ordered_idle`, and does not refuse a write. A burst of in-flight GETs still refuses a write behind them. Counting undrained `Again` inside the sixteen, as the draft suggests, refuses a boot-time mutation for a full wave of re-asks and then fails that mutation forever.

**Q4 — Releasing at completion.** Release a finished GET/HEAD from the sixteen at `complete`. Keep a finished write, storage step, native call, and module turn in the sixteen until `drain`. The 128 plus bytes bound retained results. They do not replace the write cap. Restate the completed-results test on the ordered lane for both halves.

**Q5 — The refused call.** Yes, when the ticket is actually dropped: `forget_calls` before returning the error, so `prelude.js:767-860` cannot file the fetch under `currentCall`. The path that should ship for a capacity refusal is Q8, which keeps the call. Q5 is the backstop for supersede, retirement, and a non-capacity failure.

**Q6 — The herd.** Leave LLP 1097 D3's "asked again after each delivery" (`llp/1097-storage-that-finishes-after-the-answer.rfc.md:296-300`) alone here. Boot already schedules N−2 useless re-asks because every new answer bumps `progress` (`lib.rs:774`) even though the shared fetch cannot have settled. With `Again`, those are runner settlements, one `drain` per pump (`executor_core.rs:420-421`), sitting ahead of `/labelers` and the row fetches in `ordered`. They no longer block the worker. Record the pump count in the repro. Waking only waiters whose prelude promise actually moved is the follow-up; coalescing them into one commit is a runner change (one ticket per target, `commit.rs:1052-1056`).

**Q7 — Past the window.** Refuse, one rule with reads. Re-park via Q8 so the refusal is not the death of the row. "Stay parked and retry at the next release" reorders and bypasses `forget`.

**Q8 — Refresh.** A capacity refusal of an `Again` leaves the answer pending and re-dispatches that same parked call. That does not re-enter `answer()` and does not replay earlier turns, so D2's never-restart rule does not apply. The draft's alternative "keep refusals, re-ask automatically" describes a fresh `begin`, which would replay writes. This is the parked call. Gate the retry on settlement-idle and put it at the front of the fence so it cannot spin and cannot be starved by `resume_ordered`.

## Alternatives

| Approach | Meets (a) | Fixes stuck rows | Cost |
|---|---|---|---|
| **(a) + `Again` pre-completed, write cap unchanged until drain** | Yes | Up to 128, and past 128 if Q8 re-parks | The herd still occupies the settlement queue |
| **(a) alone: re-ask admitted as a read, still a worker no-op** | Yes | Up to 128 | Every wave is serialized on the one worker ahead of the next real fetch; starting the job charges the response ceiling |
| **Refused waiter stays pending and is re-asked when the lane idles** | No. The fence still goes up, and the refusal still poisons the wave | Only if the retry keeps its place | Livelock if the lift fills the sixteen before the retry |
| **Bypass admission** | No | Yes | `resume` can write the store and issue requests. Settlement order against earlier effects goes away |
| **Coalesce one wave into one commit** | As a companion | Cuts O(deliveries × waiters) pumps to O(deliveries) | Changes the one-completion-per-pump rule and one-ticket-per-target bookkeeping |
| **Raise the sixteen** | No | Moves the cliff by a constant | D2 asked for measured bounds |
| **App shares the resolved value** | No | The clone can do it | The web runs the shared promise as written |

(a) alone is an acceptable smaller patch if the worker no-op is measured and cheap. It still fixes the permanent refusal. Pre-completing `Again` is the version that matches "a re-ask never goes to the executor" without giving up order. The runner-only retry is the overflow policy (Q8), not the admission rule.

## Risks

- **Write starvation during a wave** if `Again` is counted inside the sixteen until drain. A mutation issued at boot is refused and `release_failed` keeps the old value forever. Keep `Again` out of that threshold.
- **Irreversible effects ahead of the UI** if completed writes leave the sixteen at `complete`. Cap stays 16 until `drain`.
- **Herd latency.** Each delivery of the shared load, then each row completion that still has waiters, appends one `Again` per waiter. Twenty rows and two shared fetches are tens of extra pumps before row bodies drain, on the order of a few hundred milliseconds if each pump waits for the next turn, not a deadlock. The worker is free to run `/labelers` during those pumps only if `/labelers` was admitted before the wave that sits ahead of it. Issue order still gates `drain`.
- **Memory.** 128 `Again` outcomes are small (empty body). The unbounded case is Q7's parked map plus the live JS calls, which already exist per row. The 64 MiB waiting-buffer cap and the 4 MiB request cap stay on real requests.
- **Orphan fetch / cross-answer claim** until Q5 or Q8 lands. Shipping (a) without Q8 leaves this bug at the 129th waiter.
- **Agent drives.** Ticket order stays deterministic, so a drive that waits until `settle` converges. Intermediate snapshots change: rows that used to stick now load, and a drive that asserted the skeleton will fail. That is the intended change. The current repro sleeps 30 ms inside the transport and 1 ms in the pump (`js/tests/it/admission.rs:54-55`, `146`), so it is a macOS integration check, not the determinism proof.

## Ranked concerns

**Blocker**

- Draft (b)'s "every job releases its sixteen at `complete`" (`llp/1041-graceful-overload.rfc.md:648-657`) lets a new write run while completed writes are undrained, and it makes `ordered_idle` true too early. Release only a finished GET/HEAD from the sixteen; gate refusal settlement on the ordered queue being empty.
- `Again` must not use `reservation`'s response ceiling. Charge retained bytes of the empty outcome.
- A refused `Again` must not take the `lib.rs:896-903` path as written. That removes the parked call and fails the resource forever, and it leaves the prelude call alive to fetch under another answer (`prelude.js:1484-1490`, `1247-1250`).
- `ordered_idle` has to count undrained `Again` tickets. Otherwise the fence lifts and a refusal jumps them.

**Should-fix**

- Q3 as drafted (count every undrained `Again` inside the sixteen) refuses boot-time writes and background rounds for a whole wave. Keep the split above.
- Q7's stay-parked overflow. Refuse, and re-park through Q8 at the front of the fence.
- Q8's rationale in the draft ("automatic re-ask contradicts D2") describes restarting `answer()`. Re-dispatching the parked call does not. Specify that, or the implementer will call `release_failed`.
- Q1's "a misdeclared POST loses only its own refusal protection." A flag that admits it as a read executes the write. Do not add the flag.
- The host list omits the terminal match and the other exhaustive `Dispatch` matches. `Again` is a new variant; they have to compile.
- The repro never checks that each row's body came from its own URL, so the orphan-claim bug can pass a green "all rows show text." Assert the transport log is `/prefs`, `/labelers`, and `/row/0`…`/row/19` once each.
- `hiddenFirst` storage fan-out is outside this fix. Say so in the amendment, and do not treat a green `shared-load` test as a green Home feed.

**Nit**

- `lib.rs:687-692` is where `WAITING` is parsed; the park is `994-1006`.
- The twenty-row test is wall-clock and macOS-only. Keep it, and add a deterministic Linux/core twin that pumps `Core` without `thread::sleep`.
- Render's `Busy` outcome should be named next to the presenter's "keeps its last value," so the two failure modes are not described as one.

## Tests the implementation owes

- Un-ignore `twenty_answers_awaiting_a_shared_load_all_load`. Assert every row's text and that the scripted transport saw exactly those 22 URLs (20 rows + prefs + labelers), none of them twice.
- A deterministic ordered-queue test, no sleeps: one commit issues POST, GET, `Again`, POST. Fulfill order equals issue order. The first POST is sent before the second. The `Again` makes no transport call and leaves `jobs` and `running` unchanged.
- Seventeenth GET and seventeenth `Again` are admitted. Seventeenth POST, storage continuation, native call, and `Work::Later` are each refused. The request after the refusal stays fenced and is admitted only after the refusal settles (`a_burst_past_the_limit_refuses_one_and_runs_the_rest` still expects 17 of 18 opaque continuations).
- Sixteen completed-but-undrained ordered POSTs: the next POST is refused and still counted in the byte budget. A completed GET does not hold that slot; it still holds retained bytes until `drain`, and 128 completed GETs refuse the next GET.
- 128 `Again` tickets admitted; the 129th refused alone; the one after it held; the refused waiter re-parks, loads once the window has room, and never fetches on a neighbor's ticket. Forgetting it mid-wait drops the outcome, releases the count and the bytes, and `forget_calls` runs.
- A background storage round issued while 20 `Again` tickets are undrained is admitted. The same round issued while 16 POSTs are outstanding is refused and settles through `background_landed`, as now.
- D4 unchanged: a forgotten queued GET is never sent, a running GET aborts, a forgotten POST still runs. Retirement drops completed `Again` outcomes without joining a worker on the calling thread.
- Render of the shared-load fixture reaches `Settled::Complete`. Web fulfills `Again` on the immediate path and the row resumes.
- One scripted pump loop with a fixed clock (no `sleep`) loads all twenty rows in ticket order. Record how many pumps that took, as the baseline for Q6.
