# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 5, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `71d8099659`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Both earlier blockers are fixed. One new blocker remains: on Apple and Linux a forgotten reply never reaches `fulfill_measured`, so a retired fetch record lives until the 256 cap evicts it, and a still-running superseded POST can no longer be cancelled.

## (a) Re-check 4

Both are fixed.

The wasm keep-two table is gone. `js/web` stores the realm ticket on the request and reports door 22 tickets as they are (`js/web/src/lib.rs:436`, `js/web/src/lib.rs:792-803`). The runner keeps a replaced request as `(fetch, runner ticket)` and matches an abort against that list (`runner/src/runner/admission.rs:117-135`, `runner/src/runner/admission.rs:157-163`). Three sends leave the first two retired; aborting the first still yields its runner ticket. `an_abort_of_a_request_two_sends_back_still_cancels_it_until_its_reply` covers that (`runner/src/runner/abort_tests.rs:84-120`).

A discarded worker reply now wakes. `publish_aborts` wakes on every non-empty batch, after a turn and after a let-go (`data/src/placed.rs:87-102`, `data/src/placed.rs:395`, `data/src/placed.rs:430`). Both placed tests wait on that wake (`data/src/placed.rs:1064-1074` and the let-go test just below). The wake is the executor's waker (`runner/src/runner/time.rs:159-161`, `host/linux/src/host.rs:775`), and the pump collects before it looks for outcomes (`host/linux/src/presenter/pump.rs:17`, `host/apple/src/abi/pump.rs:12`).

## (b) Wrong request

Ticket-only matching does not select a different request in the in-tree composition.

One engine numbers fetches from 1 upward (`js/src/prelude.js:435`, `js/src/prelude.js:677`). A live reload builds a new runner: `Module::load` does not replace an engine that is already up (`js/src/lib.rs:511-514`), `activate_for_validation` runs on a candidate that is then booted as a new host (`host/linux/src/presenter/delivery.rs:179-201`), and the web swap replaces the host (`host/web/src/abi.rs:345-346`). `retired_fetches` starts empty with that runner (`runner/src/runner.rs:872`).

Rust requests never set `Request.fetch`. The only assignments are the two JavaScript engines (`js/src/lib.rs:1059`, `js/src/lib.rs:1343`, `js/web/src/lib.rs:436`). `Mixed::take_aborted` appends the Rust side (`data/src/mixed.rs:565-568`), and every in-tree Rust source leaves the default empty (`runner/src/runner/source.rs:447-448`). A Rust pending row has `fetch: None`, so a JavaScript ticket does not select it.

A pending continuation or stream is skipped (`runner/src/runner/admission.rs:120`). Continuations use `0` or `u64::MAX - 1/2/3` and do not set `fetch` (`js/src/lib.rs:179-189`, `js/src/lib.rs:1049-1059`). Streams do not call door 22 (`js/src/prelude.js:563-564`). On the web a stream does not set `fetch` either (`js/web/src/lib.rs:427-437`).

An untargeted tool fetch is not in `pending`. It is stored on the invocation (`runner/src/runner/modules/invoke.rs:341-351`). Its abort stays in `heard_aborts` until the 64-collection age-out (`runner/src/runner/admission.rs:146-151`). It does not attach to another request. The previous module table also skipped a fetch with no target.

A refused commit restores `pending` from the checkpoint (`runner/src/runner/commit.rs:180`) and does not roll `retired_fetches` back. Matching prefers the restored pending row, and that row still carries the same runner ticket (`runner/src/runner/admission.rs:125-128`). `take_aborted_answer` then decides from whatever is pending (`runner/src/runner/admission.rs:188-193`).

## (c) Retired records

They evict wrongly on Apple and Linux.

The only removal is `fulfill_measured`, which drops the row for that runner ticket before it notices the ticket is no longer pending (`runner/src/runner/commit.rs:1186-1187`). A forgotten write still runs, and its outcome is not drained: `complete` returns without enqueueing when `run.forgotten` is set (`host/apple/src/executor_core.rs:951-960`), and `forget` drops a completed outcome undrained (`host/apple/src/executor_forget.rs:7-9`, `host/apple/src/executor_forget.rs:54-72`). Linux and the render host use that same core (`host/linux/src/executor.rs:8-9`, `host/render/src/executor.rs:107-108`). `a_forgotten_write_still_runs_but_its_outcome_is_not_drained` shows `drain()` empty after the write (`host/apple/src/executor_tests.rs:786-802`).

So every superseded fetch occupies a slot until 256 newer `note_retired` calls delete the oldest (`runner/src/runner/admission.rs:157-163`). The cap is not "256 still in flight". Finished forgotten GETs hold slots too, because their replies are also dropped. After 256 supersessions, a superseded POST that is still running has no record. Its abort stays in `heard_aborts` and ages out uncanceled. The web page does call `exact_fulfill` when that POST completes (`host/web/glue.js:961-970`, `host/web/glue.js:757-758`), so the web reclaims the slot at the reply. The runner test does not catch the native hole: it calls `fulfill` itself (`runner/src/runner/abort_tests.rs:107-116`).

## (d) Wakes and new strands

No wake loop. `publish_aborts` returns on an empty batch, and each ticket is taken once (`data/src/placed.rs:92-94`, `js/src/abort.rs:9-10`, `js/web/src/lib.rs:792-803`). The native wake writes the executor pipe. The web `abortWake` coalesces on `wake.due` and the realm splices its list (`host/web/glue.js:1373`, `host/web/module-glue.js:382`). A pump that finds `module_aborts` notifies once (`host/linux/src/presenter/pump.rs:39-41`); the next pump takes the answer.

The new strand is the one in (c): an abort whose retired row was evicted. Tool fetches remain uncancelled, as they were when the module required a target.

Blocker: on Apple and Linux a forgotten reply never reaches `fulfill_measured`, so the 256 cap evicts a still-running superseded POST and a later abort does not cancel it.
