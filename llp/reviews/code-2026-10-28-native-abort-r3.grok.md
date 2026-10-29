# Code review: native abort (a fetch the app aborts is cancelled on every target), round 3 of 3, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `467d4ee011`.
- **Method:** one brief (sha256 `0ed929d0c939aa2bb0a48c6542dcfad801640e77929dfbea90c071fecb090488`), blind to the other reviewer; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

One should-fix. An abort that is published off the thread already inside the commit is kept, but nothing wakes the host, so a fetch that never completes is never cancelled and its answer never resumes. Wrong matches, withholding, and explicit cancel of a retired ticket are clean.

## 1. should-fix — a let-go abort on a worker, and a web let-go continuation, is never woken

The record is not dropped. It sits until some later, unrelated pump. If that fetch is the only outstanding work, no pump comes: the exchange stays open and the answer does not resume `Aborted`.

**Native worker placement (Apple, Linux, Android, render).** `Placed::forgotten` queues `Job::Forgotten` and returns without waiting (`data/src/placed.rs:769-781`). The owner later runs the let-go, takes the aborts, and pushes them into the mutex (`data/src/placed.rs:359-372`). That arm sends no reply and does not notify the executor. An `Answer`/`Resume` does publish before `reply.send` (`data/src/placed.rs:405-410`), so a turn that still produces a reply is collected when that reply is applied.

The host collects only on its own thread, at the end of the commit that already returned from `forgotten`:

- Apple: `host/apple/src/abi.rs:314-316` collects, then `notify` only if `has_module_aborts` is already true.
- Linux, which Android drives through the same presenter: `host/linux/src/presenter.rs:865-867`, same check. The pump is the executor fd (`presenter.rs:1334-1360`); an empty wake is not an abort wake.
- Render: `host/render/src/lib.rs:797-803` collects inside `settle`, then `executor.wait` (`host/render/src/executor.rs:116-126`) sleeps on the core's condvar. A publish into the mutex does not signal it. At the deadline, `settle` returns `Settled::Deadline` (`host/render/src/lib.rs:846-848`) without collecting again.

`forget_calls` does drain the engine before it returns (`js/src/turns.rs:81-94`), so the door-22 ticket exists before `take_aborted`. The test `an_abort_made_on_the_owner_is_handed_over_even_when_its_turns_reply_is_discarded` (`data/src/placed.rs:1015-1028`) checks that handoff after a manual `take_aborted`, not a host pump.

A let-go on the main thread is clean: the same drain runs before `forgotten` returns, and the emit above collects it in that commit.

**Web, both realms.** Door 22 only stores the ticket. Nothing enters wasm.

- Dedicated worker: `host/web/module-worker.js:50` posts `{op:'aborted'}` as the door runs. The page handler pushes and returns (`host/web/module-glue.js:447`). `forget` posts no turn result (`module-worker.js:252`). `collect_aborts` runs only inside `complete` (`host/web/src/host.rs:1026-1054`). An in-turn abort is fine, because the result message is posted after the door (`module-worker.js:264`) and fulfillment calls `complete`.
- Page iframe: `forget` calls `__exact_forget` and returns (`host/web/module-glue.js:409-426`, `221-225`). The prelude rejects the fetch promises and does not call `abortRequest` itself (`js/src/prelude.js:1298-1306`); a continuation that then aborts a different fetch is a browser microtask. `forget` does not take the checkpoint `module-glue.js:21-23` uses elsewhere, and `collect_aborts` has already run inside the wasm call. The microtask only pushes onto `aborted` (`module-glue.js:135`).

A synchronous `abort()` inside a turn the runner is still in is collected on the way out. The scheduler does not paper over the gap: with no timer due and no frames, `timerWake` schedules nothing (`host/web/timer-glue.js:4-6`, `16-25`).

## (b) Wrong match — clean

`match_aborts` (`runner/src/runner/admission.rs:113-132`) keeps a heard `(target, module ticket)` only when the one pending row for that target has that `fetch`, no continuation, and no stream. `enqueue` removes any previous pending row for the target before the new one is pushed, then matches (`runner/src/runner/commit.rs:854-859`, `917`). The keep path returns without matching (`commit.rs:824-849`), and equality includes `fetch`, so a new ticket cannot keep the old row.

Module tickets start at 1 and only increment (`js/src/prelude.js:677`). They restart in `Module::unload` (`js/src/lib.rs:707-708`), which runs on a validation candidate (`js/src/source.rs:180-187`) or on drop, not on the live runner. A dev reload boots a new host and replaces the executor (`host/linux/src/presenter.rs:624-655`), so `heard_aborts` does not survive. Rust answers carry `fetch: None` (`logic/abi/src/lib.rs:502`). Mixed reads the JavaScript source, then Rust (`data/src/mixed.rs:565-568`); one JavaScript engine owns the ticket space. An aged or capped record (`admission.rs:142-147`) is an unmatched abort, and the same ticket is not issued again in that engine, so it cannot attach to a later request.

## (c) Withholding in `take_requests` — clean

`take_requests` drops an aborted ticket before `hold_data_timers` (`runner/src/runner/commit.rs:948-955`). Timer holds, wakes, and landings are then kept only while `holds` (`runner/src/runner/advance/data_timers.rs:57-68`). A timer or continuation has `fetch: None` or `continuation: Some`, so `match_aborts` does not put its runner ticket in `module_aborts`.

The abort answer is taken before tool and admission refusals, and it is taken when `allow_ordered` is false (`admission.rs:194-195`). `has_ordered_request_refusals` (`admission.rs:296-304`) still fences those other refusals. An aborted request that was never handed out never takes an ordered place, so a later request in that batch is not stepping past a write that was sent. A request already on the wire is cancelled and its lane count drops when the forgotten run completes (`host/apple/src/executor_core.rs:1069-1077`). The remaining limit is the one the docs state: another answer's turn waits behind an ordered fetch that was sent, so a fetch another answer may abort is `exactIndependentHttp` (`docs/reference.md:1036-1041`).

## (d) Explicit cancel of a retired ticket — clean

`take_aborted_answer` always pushes the runner ticket, and skips the answer when it is no longer pending (`admission.rs:166-180`). `cancel` treats those tickets as not held and passes them as `by_app` (`host/apple/src/executor_core.rs:683-686`, `736-747`), so a write an earlier `forget` left running is aborted. A later `forget` sees `forgotten` and does not abort again. Queued explicit jobs are removed; a completed-but-undrained outcome of that ticket is dropped; a ticket already drained is absent and is a no-op. Streams are excluded (`executor_async.rs:1282-1289`). Runner tickets are not reused in one runner. Reload replaces the executor (`presenter.rs:655`).

On the web, `case "abort"` aborts that ticket's controller and deletes it (`host/web/glue.js:714`). `AbortController.abort` after completion is a no-op, and `p.finally` removes the map entry only if it is still that controller (`glue.js:760`). The rejection's `safelyFulfill` is dropped once the runner no longer holds the ticket (`runner/src/runner/commit.rs:1206-1211`); `fulfill` also ignores a stale incarnation (`glue.js:962-964`). A withheld ticket was never entered in `byTicket`, so the cancel is a no-op.

## (e) Elsewhere — clean

Same-action abort is heard before `enqueue` matches it, in that commit, before the host ages `heard_aborts`. Door 16 withdraws an unsent fetch; a stream returns without door 22 (`js/src/prelude.js:563-570`). `Module::unload` clears parked door-22 tickets (`js/src/lib.rs:705-708`). The ibex lookup returns `Aborted` or `TimedOut` on the signal or the deadline and does not start a lookup that is already expired (`vendor/ibex/crates/ibex2/src/transport/lookup.rs:138-145`, `179-184`).
