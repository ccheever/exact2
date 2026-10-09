I'll start with AGENTS.md and the round-3 review notes, then check the tip commit only against those findings.Round 3 named two blockers, a stream-signal should-fix, a UTF-8 nit, and three Grok nits. I'll check the tip commit against only those.The tip commit moves the native read off the worker and threads one deadline through both web carriers. I'll read those paths against the round-3 fixes and the send/abort/leak cases.The JS fetch path on disk does not match what I read a moment ago. I'll re-read the committed tip and confirm the worktree is still the commit under review.HEAD moved to a follow-up that caps native readers and moves the pre-abort check. I'll confirm round 3 against that tip, including whether the cap or the chunked stop drops a send or leaks a reader.All seven round-3 findings are resolved at the tip. Nothing blocks.

1. **Native cancellation and timeout.** Resolved. `acquire` in `host/apple/src/executor_body.rs:102` polls the deadline and the abort every 5 ms and returns without waiting on the read. A late result is dropped. The reader is told via `cancel` and stops at the next 1 MiB chunk (`executor_body.rs:219`). `execute` checks again before `fetch_request` (`host/apple/src/executor_core.rs:947`). Stuck readers hold a slot until their thread ends, and a fifth is refused (`executor_body.rs:86`, `executor_body.rs:111`). The stream opener passes the same abort check (`host/apple/src/executor_stream.rs:68`).

2. **Web deadline before transport.** Resolved. The page stamps one instant before the file-grant check (`host/web/http-body.js:181`) and checks it again immediately before `fetch` (`http-body.js:219`). The JS target records `started + ms` before the read (`host/web-js/ts-fetch.js:41`) and `fetchWith` checks that instant after its grant work, immediately before `browserFetch` (`host/web-js/admission.js:92`).

3. **Stream signal.** Resolved. `host/web-js/ts-stream.js:31` takes `init.signal` when it is present and the `Request` signal otherwise, aborts the controller if that signal is already aborted, and forwards later aborts before `streamed` reads the file.

4. **UTF-8 count.** Resolved. `js/src/prelude.js:655` counts a surrogate pair as 4 and a lone surrogate as 3, matching `TextEncoder`. `"app:/data/" + "\ud800".repeat(1500)` is 4,510 bytes and is rejected at `prelude.js:812`.

5. **Grok: `Request` to `wss:`.** Resolved. `host/web-js/ts-fetch.js:21` tests `input.url` for a `Request`.

6. **Grok: pre-aborted read.** Resolved. `host/web-js/ts-fetch.js:43` throws before `readBodyFile` and before the race promise is built.

7. **Grok: baked `typescriptRuntime`.** Resolved. `bake/src/receipt.rs:782` asserts `artifact_graph`'s `requires.typescriptRuntime`, the value copied at `receipt.rs:508`.

A request still inside its deadline is handed to the transport with its bytes. Timeout and abort settle the caller while the read is in flight, and those bytes are not sent. Reader slots return to zero when the thread ends. An abort returns from the wait within one poll.

No findings.

Nothing blocks.
