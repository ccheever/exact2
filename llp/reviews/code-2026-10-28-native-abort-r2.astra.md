# Code review: native abort (a fetch the app aborts is cancelled on every target), round 2 of 3, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `7fb398f447`.
- **Method:** one brief (sha256 `761a6ed64b9b10cb190a14efc93cc1b65db48e4403a2a25e69a376d1bec0b7bb`), blind to the other reviewer; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Reviewed exactly `1df7ab7829..7fb398f447`. No files changed; no builds or tests run. The round-1 wrong-next-fetch race is fixed, but abort delivery still has gaps.

1. **Blocker — an abort can arrive before its fetch enters `pending`, and is discarded.**  
   [admission.rs:104](runner/src/runner/admission.rs:104) destructively drains notifications and retains only matches already in `pending`.

   This needs no threading: an action can send `holding("x", "POST")`, then send `abortHeld("x")`. The first request remains in the action’s `later` list ([commit.rs:443](runner/src/runner/commit.rs:443)); the second send drains its abort ([queue.rs:411](runner/src/runner/queue.rs:411)); enqueue happens afterward ([commit.rs:642](runner/src/runner/commit.rs:642)). The already-aborted POST is then handed out.

   Worker placement has the same race: the owner publishes aborts before replies ([placed.rs:398](data/src/placed.rs:398)). If turn A yields fetch F and turn B aborts F before A’s envelope lands, fulfillment drains B’s notification at [commit.rs:1370](runner/src/runner/commit.rs:1370), before registering F at line 1411. Preserve unmatched notifications until registration or definitive retirement.

2. **Blocker — retiring the target suppresses physical cancellation, including POSTs.**  
   [admission.rs:137](runner/src/runner/admission.rs:137) skips both the synthetic answer **and** `explicit_aborts` when the runner ticket has disappeared.

   For example, abort a pending POST and replace its mutation with an immediate answer in the same action. Replacement removes the old pending ticket ([commit.rs:580](runner/src/runner/commit.rs:580)). Ordinary executor forgetting preserves queued writes ([executor_core.rs:700](host/apple/src/executor_core.rs:700)), so that POST can still be sent. A running POST also remains running. Furthermore, the `!run.forgotten` guard at [executor_core.rs:736](host/apple/src/executor_core.rs:736) prevents a later explicit cancellation from upgrading that ordinary forget. Transport cancellation must survive retirement independently of answer delivery.

3. **Blocker — wasm still loses aborts carried by discarded replies.**  
   Both realms remove the sole copy with `aborted.splice(0)` when finishing a turn: [module-glue.js:230](host/web/module-glue.js:230), [module-worker.js:135](host/web/module-worker.js:135). Rust reads that field only while parsing the reply ([js/web/src/lib.rs:359](js/web/src/lib.rs:359)).

   If an aborting turn is superseded while awaiting storage, its eventual result is discarded before parsing ([commit.rs:1200](runner/src/runner/commit.rs:1200)). Its abort of another live fetch disappears with it. The page’s synchronous `aborted` operation cannot recover an array already drained into that discarded result.

4. **Should-fix — worker cancellation turns cannot run behind a hanging ordered fetch.**  
   Worker answers become continuations ([placed.rs:266](data/src/placed.rs:266)), which use the ordered lane ([request.rs:750](runner/src/request.rs:750), [request.rs:371](runner/src/request.rs:371)). Sending the job to the module owner happens only when its `Work::Later` executes ([placed.rs:808](data/src/placed.rs:808)).

   The async executor refuses to start that handoff while an ordered HTTP exchange is blocking ([executor_async.rs:479](host/apple/src/executor_async.rs:479)); the synchronous executor’s ordered worker likewise remains occupied. Thus a worker-placed `holding()` followed by `abortHeld()` cannot cancel a default ordered fetch until that fetch ends. The cancellation turn needs a route that can execute while the exchange waits.

5. **Should-fix — moving the wake check later does not collect aborts created during dispatch/release.**  
   A main member of an ordered set executes application code during dispatch ([mixed.rs:391](data/src/mixed.rs:391)). Neither `dispatch_work` nor `release_work` subsequently calls `hear_aborted` ([commit.rs:955](runner/src/runner/commit.rs:955), [commit.rs:1004](runner/src/runner/commit.rs:1004)).

   The new check at [abi.rs:313](host/apple/src/abi.rs:313) therefore sees an empty runner queue even though the module holds an abort. Its result envelope can be queued behind the very ordered fetch it aborted, leaving no completion to prompt collection. Linux has the same check at [presenter.rs:864](host/linux/src/presenter.rs:864).

6. **Should-fix — the shared worker list still lacks a reliable wake independent of reply delivery.**  
   Publishing at [placed.rs:401](data/src/placed.rs:401) only extends the list. A forgotten completion wakes the synchronous host only when the ordered count reaches zero ([executor_core.rs:1068](host/apple/src/executor_core.rs:1068)); the async equivalent also conditionally wakes ([executor_async.rs:1118](host/apple/src/executor_async.rs:1118)).

   Even when that wake occurs, Linux can return from an empty pump before request collection drains the source ([presenter.rs:1360](host/linux/src/presenter.rs:1360)). The abort survives in memory but can remain unanswered until unrelated activity. The new placement test directly calls `take_aborted`, so it does not exercise this delivery gap.

7. **Should-fix — retirement turns can create aborts but never publish them.**  
   `Job::Forgotten` calls `forget` and immediately continues, bypassing the new drain ([placed.rs:359](data/src/placed.rs:359)). Forgetting explicitly runs rejected fetch continuations ([turns.rs:76](js/src/turns.rs:76)). Consequently, retiring A can execute `finally { controllerB.abort(); }` while B remains live, with no notification handed to the runner.

   The wasm worker has the corresponding omission: its `forget` operation produces no result message ([module-worker.js:253](host/web/module-worker.js:253)), and only ordinary turn results publish the abort array. Having no background rounds does not exclude these retirement continuations.

8. **Should-fix — the render host forwards aborts but never settles or cancels them.**  
   [source.rs:201](host/render/src/source.rs:201) forwards `take_aborted`, but rendering only drains executor outcomes ([lib.rs:794](host/render/src/lib.rs:794)); it never consumes `take_aborted_answer`. Its handoff also only calls ordinary `forget`, without explicit cancellation ([lib.rs:855](host/render/src/lib.rs:855)).

   A render-time resource that aborts another fetch therefore waits for the real reply or render deadline instead of rendering the rejection/fallback.

9. **Nit — the ordered-capacity documentation contradicts the async implementation.**  
   [reference.md:1036](docs/reference.md:1036) says the ordered lane itself becomes available only at physical transport end. The async executor releases its blocking slot at terminal settlement ([executor_async.rs:1093](host/apple/src/executor_async.rs:1093)). The new test deliberately starts the next request before ending the cancelled transport ([executor_async_tests.rs:385](host/apple/src/executor_async_tests.rs:385)). Distinguish that execution slot from retained bytes and transport capacity.

10. **Nit — the new panic regression test still has an unbounded wait.**  
    [lookup.rs:404](vendor/ibex/crates/ibex2/src/transport/lookup.rs:404) calls `POOL.run(..., None, ...)` directly on the test thread. If the `Ended` notification regresses, this test hangs instead of failing within a bound. Use the existing bounded helper around the operation under test.

The following areas are clean by inspection:

- **Identity on the reviewed client paths:** matching target plus module ticket, then retaining the runner ticket, fixes the original wrong-next-fetch race. Fresh fetches receive fresh module tickets ([prelude.js:677](js/src/prelude.js:677)); equal arguments do not defeat that distinction. Normal module replacement creates fresh instances and a fresh shared abort list ([placed.rs:897](data/src/placed.rs:897), [placed.rs:228](data/src/placed.rs:228)); browser completion delivery checks incarnation ([glue.js:962](host/web/glue.js:962)). I found no wrong-request collision through those reload paths.
- **Placement without an owner:** direct forwarding is correct ([placed.rs:528](data/src/placed.rs:528)).
- **Wasm double delivery and ordinary index retirement:** the two drains consume the array, and `note_aborted` removes the fetch mapping, preventing duplicate delivery. Normal finish/forget paths now unindex requests. The remaining problems are loss and notification, described above.
- **ibex2 lookup runtime:** deadline checks cover reservation and acceptance; `Slot` returns the count once and sends `Ended` on unwind ([lookup.rs:51](vendor/ibex/crates/ibex2/src/transport/lookup.rs:51)). Abort callbacks run outside the signal lock, so I found no lock inversion or double free.
- **Executor accounting once cancellation reaches it:** queued removal, terminal claiming, and retention until physical completion appear consistent. Streams remain deliberately excluded.
- **Compatibility:** runtime 5 is correctly included in compatibility identity ([compat.rs:50](bake/src/compat.rs:50), [compat.rs:523](bake/src/compat.rs:523)) and named in the docs. `dev-gallery.test.mjs` has no change in this exact diff.
