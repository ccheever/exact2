# Code review: long-poll streams (LLP 1016.000 D8), round the path sweep's final check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4151eb52a9`.
- **Method:** one brief (sha256 `6ffaaaec73f55fa7e4523b2c8a51c70538a181479a60162622027a4d415a3183`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [mixed.rs:406](data/src/mixed.rs:406), [envelope.rs:205](data/src/envelope.rs:205): A main member of an ordered set bypasses validation before encoding. The envelope drops `surface`; an otherwise valid HTTP poll carrying `surface: Some(...)` therefore reaches transport instead of refusal.
- [terminal/requests.rs:104](host/terminal/src/requests.rs:104): A valid HTTP long poll reaches `Dispatch::Missing` and `refuse_request`. Terminal never drains that refusal queue, leaving its ticket pending. The table omits this reachable kind.
- [terminal/requests.rs:117](host/terminal/src/requests.rs:117), [host.rs:330](host/terminal/src/host.rs:330): A long-poll continuation returned by `background_landed` can remain queued indefinitely. Background fulfillment returns `Ok(None)`; without another commit or announcement, terminal never calls `run_requests` to refuse it.
- [commit.rs:1111](runner/src/runner/commit.rs:1111): The known rollback defect still violates the no-pending guarantee. Terminal channel refusals, wasm batch refusals, and independent admission refusals leave `p.refused` false; a fallback whose derive traps restores a ticket with no remaining completion. Already recorded in [QUEUE.md:3](QUEUE.md:3).

**Should-fix**

- [ts-stream.js:21](host/web-js/ts-stream.js:21): An input `Request`’s body is ignored. Overriding a body-bearing POST request with `method: "GET"` permits a long poll instead of rejecting the inherited body.
- [http-body.js:373](host/web/http-body.js:373): `exact-native:` reaches the native branch before long-poll validation. The JS stream loader prevents execution, but its rejection becomes HTTP 500 `Response`, rather than `Refused`.
- [rust-data.js:103](host/web-js/rust-data.js:103): Hand-written tag-10 replies with invalid cursor/from/method apply their writes before transport validation; the native ABI rejects them before applying writes.
- [batch_tests.rs:479](host/web/src/batch_tests.rs:479) tests only the predicate, not host dispatch or settlement. [Renderer test:46](host/render/tests/it/longpoll.rs:46) allows `Deadline` and never verifies the retained placeholder.

**Nits**

- [abi.rs:274](host/apple/src/abi.rs:274): The table omits `exact-activity:` work; the preceding guard covers it.

All six permitted tests were blocked before execution by denied Cargo writes. No files changed.
