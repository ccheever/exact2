# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 4, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `a581a720f0`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Both fixes hold. A refused commit restores the request, and the next pump both resumes it and cancels it. With no rollback, the host only cancels.

**(a) Both fixed.**

The web module records the fetch, then hears, then prunes. `js/web/src/lib.rs:507-522` pushes `(ticket, key)` and sets `request.fetch` before `hear_aborted()`. An abort that is already in the realm's list, including one another answer made of this just-claimed fetch, is matched while that entry exists. `note_aborted` (`js/web/src/lib.rs:184-197`) keeps a ticket that is not in `fetches` yet on `unmapped` and retries it on every hear. `hear_aborted` (`js/web/src/lib.rs:216-227`) still calls `note_aborted` when the module is not ready or `realm("aborted")` fails, passing an empty list. The test `an_abort_heard_before_its_fetch_is_recorded_names_it_once_it_is` (`js/web/src/lib.rs:1024-1046`) goes through `step`, which hears after the push.

The runner no longer consumes a retired abort during the commit. `match_aborts` (`runner/src/runner/admission.rs:135-141`) pushes the runner ticket onto `module_aborts` and removes the retired row. `take_aborted_answer` (`admission.rs:202-216`) decides later: it always queues that ticket on `explicit_aborts`, and it returns `Failed { Aborted }` only when `pending` still contains the ticket. `collected_aborts` (`admission.rs:181-183`) is `has_module_aborts()` again.

Rollback after that match: `Checkpoint` (`runner/src/runner/commit.rs:18-47`) does not include `module_aborts`. `conclude` (`commit.rs:180`) puts `pending` back, so the retired request is pending again under the same runner ticket. `module_aborts` still holds it. The following pump sees that:

- Web (`host/web/src/host/complete.rs:35-60`) fulfills the abort answer, then `batch.abort`s `take_explicit_aborts()`.
- Linux (`host/linux/src/presenter/pump.rs:17-33`, then `presenter.rs:811-865`) fulfills via `take_request_refusal`, cancels in `service_commit`, and `collected_aborts()` notifies while `module_aborts` is still set, including after the commit that just failed.
- Apple (`host/apple/src/abi/pump.rs:12-42`, `abi.rs:257` and `abi.rs:312`) does the same: fulfill, then cancel, and notify if any abort remains.

So the restored request is resumed with `Failed { Aborted }` and its transport is cancelled. If that answer commit itself is refused, `fulfill_now` (`commit.rs:1260-1275`) still `release_failed`s a ticket the rollback put back, and the explicit cancel still runs.

No rollback: the replacement stays out of `pending`. `take_aborted_answer` queues only the cancel and returns `None`. `runner/src/runner/abort_tests.rs:64-74` asserts `module_aborts` is set, `explicit_aborts` is empty until `take_request_refusal`, there is no answer, and the cancel is the first request's runner ticket.

**(b) `unmapped` does not name the wrong request, and keep-two does not drop a mapping an abort still needs.**

Prelude tickets start at 1 and only increment (`js/src/prelude.js:435`, `677`). One module has one realm, so a live `unmapped` entry cannot meet a reused number. The realm reports each door-22 ticket once (`host/web/module-glue.js:382`, `aborted.splice(0)`). `note_aborted` binds a ticket to whatever key was stored with it (`js/web/src/lib.rs:190-193`). That key is the call `step` is handling (`lib.rs:302-303`, `507-510`), and the turn result's `ticket` is that call's fetch. A ticket with no row yet stays on `unmapped` and does not attach to some other key.

Keep-two runs after the hear (`lib.rs:514-522`), newest first. The recording step therefore matches an `unmapped` ticket, removes it from `fetches`, and puts it on `aborted` before the prune. The entry the prune can drop is an older fetch on that same key, and only once two newer ones are already recorded. The abort of the fetch this call replaced is door 22 during that turn, already in the realm list when this step hears (main thread pushes synchronously; the worker `postMessage`s the abort before the turn result). That is the same one-previous window as before.

**(c) No wake loop and no stranded abort.**

`take_aborted_answer` always removes the runner ticket from `module_aborts`. Hosts wake while it is non-empty (Linux `presenter.rs:864-866`, Apple `abi.rs:312-314`, web `abortWake` to `exact_collect_aborts`) and take `explicit_aborts` in that same pump, including the cancel-only path (`pump.rs:42-45`, `complete.rs:58-60`). The realm list and `Module::take_aborted` (`lib.rs:879-884`) are both drained, so a heard ticket is not reported again. `unmapped` never wakes the runner. It is retried on each hear and dropped after the cap (`lib.rs:195-200`).

**(d) Nothing new.**

No new client-reachable strand, wrong ticket, or loop in this increment.

clean
