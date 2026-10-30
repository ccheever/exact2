# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8117d6245f`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Its worktree was moved to 3a0115739e while it ran, so its later reads saw that tree.

Both blockers are fixed. The parked-only table holds for the four cases, and the line-cap moves are pure.

### (a) The two blockers

**The 256 cap is gone, and a live parked fetch stays named.** `FETCHES_KEPT` and the `remove(0)` eviction are gone. `park` records a fetch only when the answer has a target, the request carries a fetch ticket, and it is not a stream (`js/src/lib.rs:1367-1371`). `take_aborted` maps queued door-22 tickets while those rows still exist, then keeps only tickets that some parked answer still waits on (`js/src/abort.rs:16-26`). Door 22 only pushes the ticket (`js/src/door.rs:232-237`). The new test dispatches one held POST, then 300 replacing GETs on a different mutation, and asserts the abort still names the POST (`js/tests/it/abort.rs:314-337`). That is the old eviction. It was not executed here.

**A replaced call's abort in the replacing turn is still reported.** `begin` runs the script first, and only then drops the old park and calls `forget_calls` (`js/src/lib.rs:1074-1093`). The prelude sends door 22 only for a fetch settle has already claimed (`js/src/prelude.js:563-566`), so the row is already in `fetches`. `forget` unsubscribes that fetch before it rejects (`js/src/prelude.js:1292-1299`), and `forget_calls` drains only after those unsubscribes (`js/src/turns.rs:70-88`). `take_aborted` maps first and prunes second, so a ticket already queued is returned even though the old answer is no longer parked. The owner takes aborts only after `turn()` returns (`data/src/placed.rs:424`) and after a forgotten job (`data/src/placed.rs:375`). The runner hears them only after the answer returns (`runner/src/runner/admission.rs:102-107`). Nothing in that turn calls `take_aborted` early.

**A drain cannot run before that turn ends and drop the row first.** On the native engine, door 22 does not re-enter the runner. `forget_calls`'s drain runs with the listener already removed, so it cannot file a new door 22 for that ticket. On the page, the extra `setTimeout` collect can run while a turn is suspended at `checkpoint`, but door 22 is also claim-gated there, and the wasm table is filled in `step` before `complete` collects, inside the same `exact_fulfill` (`js/web/src/lib.rs:473-481`, then `host/web/src/host/complete.rs:35`). From claim to that fulfill the turn stays on microtasks (`host/web/module-glue.js:348-366`); the timer cannot land in between. A collect that wins the race against a later checkpoint is naming a fetch already recorded.

**Unload drops the lists together.** `unload` clears `parked`, `fetches`, `requests`, and `aborted`, then drops the engine (`js/src/lib.rs:695-715`). No later door 22 can arrive. The retain does not change that.

**A continuation park does not orphan a live fetch.** A wait, yield, or timer is parked on a sentinel (`js/src/lib.rs:851-856`). Those requests never set `fetch` (`js/src/lib.rs:1055-1056`), so they add no row. A targeted continuation does not remove the existing park (`js/src/lib.rs:1074`). Resuming the fetch itself deletes that ticket from `fetches` before `__exact_fulfill`, and fulfill settles the listener before the continuation (`js/src/lib.rs:1230-1233`, `js/src/prelude.js:1444-1446`). The module-wide "finished, but a claimed fetch is still in flight" park is a second entry beside the fetch park, so the retain still sees `p.ticket == fetch`.

**The page realm now wakes under a scratch context.** Door 22 always calls `abortWake` (`host/web/module-glue.js:135`), including while a let-go's delivery has installed a scratch context (`host/web/module-glue.js:214-218`). The worker path already woke (`host/web/module-glue.js:447`). `abortWake` is one `setTimeout` per turn (`host/web/glue.js:1373`): the flag is cleared before `exact_collect_aborts`, and that call splices the realm's list (`host/web/module-glue.js:382`). A second abort during the collect schedules one later pass. The resume is queued on `tail` (`host/web/module-glue.js:344-366`), so it does not start `begin` while the suspended turn still holds `context`. The test expects one wake for both placements (`host/web/module-storage.test.mjs:289`). It stubs `abortWake`, so it checks the call, not the wasm re-entry. That re-entry is safe for the ordering above.

### (b) The moves

Each body matches the old one aside from whitespace. Configurations are intact.

- Linux `Presenter::pump` is the same function in `host/linux/src/presenter/pump.rs:12-56`, still calling `collect_aborts` at the start. `abort_tests` stays `#[cfg(all(test, unix))]` (`presenter/pump.rs:4-6`) and still resolves to `presenter/abort_tests.rs`.
- Apple `Bridge::pump` is the same function in `host/apple/src/abi/pump.rs:6-43`.
- `Core::forget`, `cancel`, and `forget_with` match the old bodies. `forget` and `cancel` went from `pub(super)` to `pub(crate)` (`host/apple/src/executor_forget.rs:13-26`); `forget_with` stays private. `Core` lives in private `mod core` (`host/apple/src/executor.rs:7`, `executor_core.rs:1371-1372`), so the rest of the crate still cannot name them. The `Executor` wrappers are unchanged (`executor.rs:58-64`).
- The five executor abort tests match the old bodies. They sit in `pub(crate) mod abort` under the existing `#[cfg(test)]` tests module (`executor_tests.rs:1498-1499`, `executor_core.rs:1384-1386`). The helper was already `pub(crate)`. The Linux wire test only updates the path (`host/linux/src/executor.rs`).
- Render `hand_out` is the old function (`host/render/src/requests.rs:6-36`). `settle_aborts` is the old loop, including `fulfill_measured`'s `?` (`requests.rs:40-52`). `pub(super)` from the child is the same reach the private parent functions had.
- Web `complete` and `collect_aborts` match. `complete` is `pub(super)` so the parent can call it (`host/web/src/host/complete.rs:20`); it was private on the parent. `request_active`, `fulfill`, and `collect_aborts` keep their `pub` signatures (`host/web/src/abi/requests.rs`). `abort_tests` stays `#[cfg(test)]` (`requests.rs:6-8`).
- `glue.js` joins the abort case onto its comment (`glue.js:713`); the `byTicket` delete and `c?.abort()` are unchanged. `abortWake` is the old flag-and-timer, stored on the function.

`Runner::collected_aborts` is `collect_aborts` then `has_module_aborts` (`runner/src/runner/admission.rs:150-154`). Apple and Linux emits use it for the same notify as the two calls they replaced (`host/apple/src/abi.rs:311-313`, `host/linux/src/presenter.rs:863-866`). Both pumps still call `collect_aborts` themselves.

The Linux commit site now passes `take_explicit_aborts()` straight into `cancel` (`presenter.rs:808-811`). `cancel` runs only after that vec is taken. Same result.

### (c) Nothing else new

The docs line now says a fetch made by a module callback that answers no resource or mutation is not cancelled yet, natively and on wasm (`docs/reference.md:1045-1047`). That matches `park`, which records a row only with a target. The `take_aborted` comment matches the map-then-prune order (`js/src/abort.rs:7-12`, `23-24`).

Swappable is untouched. A replacement still installs a new host or executor and listens again: Apple `candidate.host.listen_io` (`host/apple/src/abi.rs:921`), which calls `runner.listen` (`host/apple/src/host.rs:773-775`); Linux `self.executor = self.host.executor()` (`host/linux/src/presenter.rs:656`, `presenter/delivery.rs:372`), and `executor()` calls `runner.listen` (`host/linux/src/host.rs:769-775`).

clean
