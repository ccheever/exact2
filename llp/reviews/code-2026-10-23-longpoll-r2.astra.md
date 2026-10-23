# Code review: long-poll streams (LLP 1016.000 D8), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3040641783`.
- **Method:** one brief (sha256 `0706e52d32c9ccbf41c8cc5e0d670fb47db0c267d78b9b4d1b4f43e516abe890`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- **The marker is forgeable by ordinary SSE.** [ts-stream.js:147](host/web-js/ts-stream.js:147), [rt.js:419](host/web-js/rt.js:419), [executor_stream.rs:204](host/apple/src/executor_stream.rs:204): a server’s `event: exact:reply` is silently dropped by both JS adapters and receives special native coalescing treatment. Gate marker handling on the ticket’s long-poll mode.

- **The no-stream refusal still has send paths.** [executor_async.rs:648](host/apple/src/executor_async.rs:648), [http-body.js:383](host/web/http-body.js:383): with `EXACT_HTTP_ASYNC=1`, `Request::post_json(...).long_poll(...)` without `stream` sends the POST. Likewise, a web operation carrying `longPoll` with `stream:false` falls through to ordinary fetch. Validate before transport selection.

- **The ABI test now necessarily fails.** [tests.rs:210](logic/abi/src/tests.rs:210): the empty cursor now encodes as a valid tag-4 refusal, so `read_result` returns `Ok(Err(...))`; the assertion expects outer `Err`. Update this assertion and separately test malformed tag-10 decoding. This conclusion is from source, not an executed test.

**Should-fix**

- **Native coalescing loses reply counts.** [executor_stream.rs:208](host/apple/src/executor_stream.rs:208), [commit.rs:963](runner/src/runner/commit.rs:963): a marker arriving behind another marker or event disappears without accounting. Coalesced events also increment `heldReplies` only once. Preserve reply counts separately from dropped-event counts.

- **Some JS terminal outcomes never count.** [http-body.js:200](host/web/http-body.js:200), [rt.js:410](host/web-js/rt.js:410): redirect refusal and caller abort bypass `replied()`, while completion only removes held state. An already-running `clock held` consequently times out despite the terminal mapper call.

- **Replacement Rust stream tickets lose held identity.** [rt.js:480](host/web-js/rt.js:480), [held.js:18](host/web-js/held.js:18): when parsing a message returns another request, the new ticket lacks `owner` and `name`. `longPollReplied` rejects every subsequent reply, losing held naming and counts.

- **Lone surrogates defeat cursor round-tripping.** [http-body.js:184](host/web/http-body.js:184): `{"seq":"
