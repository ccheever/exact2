# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 3, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `3a0115739e`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

The wasm same-key replacement abort now reaches the controller, a retired record cancels only the runner ticket stored with that target and fetch, and counting explicit aborts does not spin a pump.

**(a) The wasm sequence holds.** A module turn is a continuation: both realms return `{continuation}` before the turn runs (`host/web/module-glue.js:368`, worker `471–477`), so the runner has already dropped the old POST. `enqueue` records it in `note_retired` (`runner/src/runner/commit.rs:858–860`) and the same note runs for a replacing commit (`583`) and `forget_ticket` (`786`).

Door 22 lands before that turn’s result. The iframe pushes synchronously (`module-glue.js:135`, `aborted.splice` at `382`). The worker `postMessage`s the abort (`module-worker.js:50`) and posts the result only from `done.promise.then` (`264`), so the page handles the abort first (`module-glue.js:447`).

`step` calls `hear_aborted` before the per-key prune (`js/web/src/lib.rs:484–496`). `note_aborted` maps any ticket still in `fetches`, with no waiting check (`168–182`). `take_aborted` hears again, then drops mappings whose key is no longer waiting (`851–856`). The old ticket is named while it is still stored; the key stays waiting for the new fetch, so a later poll still finds the one previous ticket the prune kept.

`fulfill_inner` hears before it enqueues the new answer (`commit.rs:1382`, then `1423`). The continuation has no fetch, so the pending scan misses and the retired scan pushes the stored runner ticket into `explicit_aborts` (`admission.rs:118–136`). If the abort was matched while the POST was still pending, `take_aborted_answer` still moves that same ticket onto the explicit list once it has left pending (`198–204`).

Web `complete` drains that list into `batch.abort` (`host/web/src/host/complete.rs:35–59`, `batch.rs:225–228`). Glue aborts `byTicket.get(op.ticket)` (`host/web/glue.js:713`). A POST is not forgettable (`754`), so `letGo` (`954`) leaves its controller in place until this abort. `abortWake` (`1373`) is only the between-batches path into the same `complete`.

**(b) A retired row cancels the runner ticket it stored.** The match is target plus fetch ticket (`admission.rs:133–136`), and only after the pending scan misses.

A refused commit restores `pending` from the checkpoint (`commit.rs:96`, `180`) and leaves the retired row. The restored POST is still pending with the same fetch (fetch is assigned at `js/web/src/lib.rs:494` and never rewritten on a live request), so the next abort hits that pending row and names its runner ticket. The ghost row is not consumed and does not fire on its own.

A new runner starts with an empty retired list and `next_ticket: 1` (`runner/src/runner.rs:865`, `872`). Web boot replaces the host (`host/web/src/abi.rs:311`) and then aborts live controllers (`glue.js:1352–1353`). Prelude fetch tickets also start at 1 and only increment (`js/src/prelude.js:435`, `677`), and only inside a new realm. A late collect on the new runner finds no fetch mapping (`note_aborted` skips an unknown ticket).

The same request pending and retired matches pending first (`admission.rs:124–128`). The row’s stored ticket is that request’s ticket. After it leaves pending, a later hit still pushes that stored ticket. `byTicket` deletes on abort (`glue.js:713`), so a second push is a no-op.

**(c) No wake loop.** `collected_aborts` is collect-then-`has_module_aborts || explicit non-empty` (`admission.rs:177–179`). Apple `emit` and Linux `service_commit` drain `take_explicit_aborts` before that call (`host/apple/src/abi.rs:257` then `312`; `host/linux/src/presenter.rs:811–812` then `864`). A true result only `notify`s. The matching row is removed, so the next collect does not push it again. Linux’s empty pump drains explicit and returns without calling `collected_aborts` (`presenter/pump.rs:42–50`). Apple’s pump collects, then `emit` drains (`abi/pump.rs:12`, `42`). Web `emit` only copies the batch (`host/web/src/abi.rs:169–171`); `complete` drains explicit in the same call. Render continues the settle loop only when an answer was fulfilled (`host/render/src/requests.rs:40–51`, `lib.rs:796–798`); a retired-only cancel returns false. An empty ticket list returns before `forget_with` (`host/apple/src/executor_forget.rs:22–25`, shared with Linux). A non-empty cancel can wake one follow-up pump; that pump drains the list and the bool is then false.

**(d) Nothing new that blocks.** Keep-previous retains one prior fetch per key (`js/web/src/lib.rs:485–490`). The reported turn is safe because the abort is already in the realm list when that step hears, before the prune. The js/web test injects `note_aborted` after `step` (`997–1023`) because the realm call is inert off wasm; the poll order is the `hear_aborted` at `484` and `851`. A refused commit’s ghost row can occupy one of the 64 retired slots (`admission.rs:166–171`) until it ages out (`157–160`). That evicts the oldest record under a full list; it does not select another ticket.

clean
