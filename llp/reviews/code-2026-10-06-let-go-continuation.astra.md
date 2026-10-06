# Round 1
Found three defects:

1. **[P2] Settled waiters can remain held indefinitely** — [js/src/turns.rs:66](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:66). The drain can settle another live answer sharing the rejected promise, but never increments `self.progress`. If an unrelated stream remains open, `wake()` sees unchanged progress and outstanding work, so it keeps that completed answer held until another delivery or call occurs.

2. **[P2] Cleanup storage is incorrectly refused as bake-time work** — [js/src/turns.rs:67](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:67). The drain runs before `finish_let_go()` enables `between_answers`. On main placement, `storageCall()` assigns cleanup issued without a current answer to background work, but the host rejects it with `code: "bake"`. I reproduced a forgotten fetch’s catch attempting a storage write: the adapter never ran. An asynchronous cleanup can consequently fail before clearing its busy flag. This path also needs background-state refresh if cleanup starts work.

3. **[P2] Stream termination now produces an extra synthetic rejection** — [js/src/prelude.js:1271](/Users/admin/projects/exact2-wt-ft/js/src/prelude.js:1271). The rejection includes `p.stream` entries. Normal stream termination also calls `forget_calls`, so the terminal outcome reaches `exactStream`, then `.catch()` receives a separate `Aborted` error and `.finally()` runs. I reproduced `mapper Network → catch Aborted → finally`. This violates the documented never-settling stream promise and differs from the web build’s implementation.

No runner re-entry or claimed-fetch corruption found: transferred tickets survive, deleted calls stay deleted, and the cleared host store prevents writes into another answer’s Store. The drain remains externally interruptible, although this path neither measures its execution budget nor reports drain errors.

Validation: 22 cached native tests passed; additional in-memory prelude probes exercised the cases above. No files modified.
# Round 2
Three defects remain:

- **[P1] Calls owning both fetch and storage can skip the rejection drain** — [turns.rs:63](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:63). `__exact_forget` returns `"storage"` even when it rejected fetches. If that storage is queued behind a live answer, `finish_let_go()` returns without draining. The forgotten catch then runs during the live answer’s next drain, with its Store installed. An in-memory prelude reproduction confirmed the forgotten catch writes into that live Store.

- **[P1] Interrupted drains retain queued reactions** — [turns.rs:80](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:80). `watch.take()` clears only the interrupt flag. Hermes retains jobs after the interrupted job, and the next `begin()` drains them with the new answer’s Store installed. Remaining forgotten reactions can therefore mutate that Store or resume a recursive microtask chain. The runtime needs recovery or retirement before reuse.

- **[P2] Cleanup storage is invisible to the background scheduler** — [turns.rs:74](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:74). Storage started by this drain does not trigger `refresh_background()`. With `owed == false`, the cached background head remains empty, so no completion ticket is issued. The first operation can finish physically, but subsequent awaited writes remain stranded until another answer refreshes the state. The prelude reproduction showed cached `head:false` versus actual `head:true`.

The stream exclusion, normal-drain progress increment, and documentation corrections look correct.

Both added tests passed using the existing binary. The worker interruption test failed because interruption occurred during initialization. No files changed.
# Round 3
- **[P2] Retry exhaustion still lets forgotten reactions run in the next answer** — [js/src/turns.rs:75](/Users/admin/projects/exact2-wt-ft/js/src/turns.rs:75). After nine failed drains, the code clears the interrupt flag and returns with remaining jobs queued. The next answer drains them with its live Store installed, preserving the round-2 corruption path. Reproduced in Hermes: an aborted fetch’s catch queues nine throwing jobs followed by `store.set`; that write executes during the next answer. Exhaustion needs to prevent live answers from draining the leftover queue.

The combined storage/rejection signaling and background refresh look correct.
# Disposition

Fixed in 2dd524b9a, 80de6d788 and 302effe5b. Round 3: the wasm realm's exact 'storage' checks now accept 'storage rejected' (Grok), and the drain retries until the queue behind a failing job is empty, bounded at 1024 tries only against an endless chain of failing jobs (Astra: a chain longer than that still leaves jobs for the next answer). Not re-reviewed after round 3.
