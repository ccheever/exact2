# Code review: LLP 1146 D2, an aborted kept write in a module turn, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `7feec937aa`.
- **Method:** one brief (sha256 `eef25cd6f2ee6c3d2dceb9ae6ef96b75750cefed428c873f504dfd7e8a074866`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

An adopted fetch is left pending only when the abort hook takes the early return at `js/src/prelude.js:706` (`p.module`, still pending, and door 23 accepted). `p.module` is set only by `__exact_adopt` (`prelude.js:1460`), which runs inside `keep_sent_now` before the runner inserts `module_work.fetches` (`runner/src/runner/module_work.rs:152-158`). Door 23 only appends `state.aborted` (`js/src/door.rs:247-250`). The committing turn has already heard aborts (`runner/src/runner/commit.rs:1333`) before `keep_sent` (`commit.rs:226`), so the next hear is a later pump, after that insert. `match_aborts` prefers the kept map (`admission.rs:130-133`), which has no cap, so neither the 256-entry retired list nor the 64-collection expiry of unmatched `heard_aborts` (`admission.rs:152-159`) drops it. A reply that already settled the promise fails the `pending.get(ticket) === p` check and the hook does nothing. A reply still in flight and the abort answer share one removal in `module_work_landed` (`module_work.rs:175-178`); the second fulfill is dropped, and `__exact_fulfill` rejects from `p.signal.reason` when the signal is already aborted (`prelude.js:1472`). A refused commit restores `pending` and truncates `letting` (`commit.rs:193-194`) and leaves `module_aborts` and an already-inserted kept map in place. `unload` drops the engine with the heap (`js/src/lib.rs:719-735`). A worker-placed source returns no tokens (`data/src/placed.rs:980-988`) and `set_aside` requires the main thread (`js/src/module_work.rs:59`). The wasm realms have no `keep_sent` override, so they never call `__exact_adopt`. A door 23 throw makes `cancelAdopted` return false (`prelude.js:578-579`) and the hook rejects at once.

`cancelAwait` aborts and then awaits the same promise. The hook leaves it pending, `settle` parks the live answer while module work is outstanding, and the turn returns. The following pump takes the abort answer before the ordered-idle check (`admission.rs:233-234`, `host/apple/src/abi/pump.rs:13-16`), and `module_landed_now` rejects it. The live answer’s step is deferred and resumed after `progress` moves. The write is on the independent lane, so the ordered FIFO stays free for that later turn.

`claim` sets `p.claimed` (`prelude.js:1073`) before a fetch ticket is returned to Rust and parked (`js/src/lib.rs:1067-1092`). Nothing clears it. A parked effectful ticket is one `claim` handed out. If the new check failed, adopt would return `""`, the runner would not keep the request, and `__exact_forget` would reject the fetch.

`an_aborted_kept_write_s_continuation_writes_in_a_module_turn` asserts the store write survived the refused cancel commit (`js/tests/it/outlive.rs:458`), the `module work 1: aborted: …` line (`473`), no leftover module work (`477`), and a single `POST /token w` (`480`). Without the early return, `reject` runs in the cancel turn, inside that commit’s checkpoint, and `restore_store_refused` drops the write, so the token assertion fails. The listener uses port 0, `DRIVE` serializes the tests, and both new tests release the gate after `pump_until`, including when that wait panics. An assertion inside `kept_then_aborted` (`430`) still skips `release_gate`, and the server thread then sits until `HANG`; that shows up only after the test has already failed. The web test (`host/web-js/store-readers.test.mjs:106-120`) shows a rejection’s `store.set` surviving a refused commit on the JS target. The native module turn is the Rust test.

`docs/reference.md:1028-1040` and D2 §2.1 (`llp/1146-oauth-sign-in.rfc.md:78-88`) match the code. Native module work defers the rejection. The JS target uses the browser fetch, and `Store.set` with no commit open is kept. Wasm realms and a worker-placed source keep nothing.

`TYPESCRIPT_RUNTIME` stays 5 (`bake/src/compat.rs:47-50`). The prelude is compiled into the host, this change adds no API, and an older prelude rejects in the turn that called `abort()`.
