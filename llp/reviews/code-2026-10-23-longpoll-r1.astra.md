# Code review: long-poll streams (LLP 1016.000 D8), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c89cbfa57d`.
- **Method:** one brief (sha256 `980616989fac23f8bf2f26c95a87ff62e680603106360349d70ab313c5a9a599`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [bake/src/compat.rs:34](bake/src/compat.rs:34), [scripts/rust.mjs:421](scripts/rust.mjs:421): **(a) Move the Rust compatibility requirement.** Both still advertise ABI 3, so an older cohort can receive a module using tag 10. Runtime rejection prevents misdecoding, but a resource on a later-opened route can fail after installation. Update the ABI/receipt/artifact requirements consistently.
- [http-body.js:187](host/web/http-body.js:187), [glue.js:750](host/web/glue.js:750): **Cursor strings can inject wasm message metadata.** A cursor containing `x
coalesced: 7` enters the newline-delimited header encoding verbatim; [host.rs:58](host/web/src/host.rs:58) truncates its ID and reads the injected count. Use an encoding that preserves arbitrary strings.

**Should-fix**

- [http-body.js:152](host/web/http-body.js:152), [executor_longpoll.rs:236](host/apple/src/executor_longpoll.rs:236): Numeric cursors diverge. `9007199254740993` becomes `9007199254740992` on web but remains exact natively. That can suppress a subsequent change as a heartbeat. Preserve numeric tokens losslessly or enforce the same explicit numeric restriction everywhere.
- [http-body.js:182](host/web/http-body.js:182), [executor_longpoll.rs:193](host/apple/src/executor_longpoll.rs:193): SSE mismatch detection waits for EOF on both hosts, so a real open SSE response never promptly receives the promised refusal. Native also consumes transient-status bodies before retrying, whereas web cancels immediately: a large 503 terminates native but retries web. Classify these outcomes at headers.
- [http-body.js:172](host/web/http-body.js:172), [executor_longpoll.rs:148](host/apple/src/executor_longpoll.rs:148): Repeated `Retry-After: 0`, or past dates, bypass backoff indefinitely. **(b) Keep the 1 s heartbeat floor**, and apply a positive floor to retries without progress too.
- [http-body.js:146](host/web/http-body.js:146): `pause()` misses an already-fired abort. If cancellation happens during the awaited body cancellation, the subsequent wait installs its listener too late and retains the controller/retrying state for up to 60 s. Check `signal.aborted` when entering the wait.
- [http-body.js:166](host/web/http-body.js:166): Redirect refusal leaves the response body uncancelled, then removes its controller from tracking. Cancel the body or abort the transport before returning; otherwise the fetch can outlive the stream.
- [http-body.js:345](host/web/http-body.js:345), [rust-data.js:103](host/web-js/rust-data.js:103), [executor_longpoll.rs:72](host/apple/src/executor_longpoll.rs:72): Host validation is incomplete. A Rust tag-10 `wss:` request opens a WebSocket on web while native refuses it. Empty/oversized terms also pass direct native and web-js paths despite rejection by the native ABI decoder. Validate before transport dispatch.
- [http-body.js:153](host/web/http-body.js:153), [executor_longpoll.rs:231](host/apple/src/executor_longpoll.rs:231): `from: "a.b.c"` passes validation but selects `a.b` on web and `a["b.c"]` natively. Enforce the documented one-level path grammar consistently.
- [rt.js:398](host/web-js/rt.js:398): `sameReq()` omits `longPoll`. Changing a Rust resource’s arguments to change `cursor`, `from`, or `sameCursorEvent` while retaining its URL keeps the old poll and its controller. Include the new terms in request identity.
- [ts-fetch.js:52](host/web-js/ts-fetch.js:52): The missing-`exactStream` guard is bypassed by the mutation/socket branch at line 23, which calls `fetchWith()` directly and drops `exactLongPoll`. An invalid call can therefore send an ordinary request. Validate before that branch.

**Nits**

- [executor_longpoll.rs:95](host/apple/src/executor_longpoll.rs:95): `as_millis()` truncates the remaining heartbeat delay. Preserve the duration or round upward to honor “no sooner than” and avoid undermining the timing assertion.

Reissued requests retain grant checks; native redirects check every hop. Web’s final-URL-only redirect behavior is an existing documented limitation. Cursor query encoding does not let a reply change origin or path.

All three permitted tests stopped before assertions because the read-only sandbox denied Cargo build writes; Bun’s suite also builds a Cargo helper. No files changed.
