# Code review: the exact-js overlay cut, round 3, narrow re-check (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8a08292664`.
- **Method:** one brief (sha256 `5e1f80dac6ca3b9c886e4de9a3a093402d0e2264c825712c3d1e510e06b6e921`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Both round-3 majors are fixed in `8a08292664`, and the page-realm test passes on this tree. Nothing in the commit reopens the overlay decision or the guard.

**M1.** `Module::load` and `Module::inspect` both finish in `install` (`js/src/lib.rs:604`), which is the only place `__exact_has_overlay` runs. The probe is after the identity and grant checks, and a failed probe keeps the overlay asked (`map_or(true, …)`) and still stores the engine. `the_bake_reads_the_overlay_at_load_as_a_device_does` compares one inspected module with one loaded module: the overlay-odd proxy counts a single read before any answer, then two more for one overlay (`typeof` plus the call), and an inspected module with no overlay encodes nothing. Without `install` on `inspect`, `has_overlay` stays at its default `true` (`js/src/lib.rs:416`), so that test’s first read is `0` and the no-overlay module encodes once.

Other `inspect` callers are unchanged in result. `windows_lean` fails inside `load_engine`, before `install`. Bake, inputs, storage, timers, and entropy only gain a read of `exact.overlay`. A data property that is missing sets `has_overlay` false, and `overlay()` returns `None` without encoding — the same shown answer the prelude’s `{tag:4}` path already produced. Bake’s snapshot goes through that same `Module::overlay`.

**M2.** `__exact_overlay` now clears `currentCall` and sets `overlaying` before either read of `exact.overlay` (`js/src/prelude.js:1401`). `store.set` checks `overlaying` before the host door (`js/src/prelude.js:760`), so both reads throw `store.set() is unavailable in an overlay`. A `return` of `{tag:4}` is inside the `try`, so `finally` still restores the saved call. The page-realm test passed here (`bun test host/web/module-storage.test.mjs -t "writing getter"`, 1 pass). On the old prelude the first read runs while `overlaying` is still false, so the assertion fails: after `answer` returns, the native door has no store (`js/src/lib.rs:1023`, `js/src/door.rs:51`) and the page turn has cleared `context` (`host/web/module-glue.js:141`). The second read was already inside the guard, so only the first line is wrong. The native fixture is the same shape and was not recompiled in this pass.

What the prelude does before calling the overlay — `JSON.parse`, and the `writes` array with `keep` — does not touch the host. A plain function property is not invoked by `typeof`. The existing proxy fixture only counts the get. The overlay function itself was already called under this same guard.

**LLP 1054 and the queue.** A worker realm has neither `hasOverlay` nor `overlay` (`host/web/module-glue.js:469`). `call` therefore answers `has-overlay` with `{has:true}` and `overlay` with `{tag:4, worker:true}` (`host/web/module-glue.js:526`). `js/web` encodes the answer and the writes only after that probe returns true (`js/web/src/lib.rs:569`). The page realm still uses the load-time cache (`overlayAtLoad`). The new LLP sentence and the `QUEUE.md` line match that path, and the queue item is the init-handshake follow-up.

VERDICT: APPROVE

No findings.
