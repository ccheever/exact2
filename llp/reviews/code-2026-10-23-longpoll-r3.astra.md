# Code review: long-poll streams (LLP 1016.000 D8), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `5634a624cd`.
- **Method:** one brief (sha256 `06cc06e33749731b416a4d2433f18e8ea0ac5ecab51f19d1b432d1fab526cdce`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [executor_core.rs:1332](host/apple/src/executor_core.rs:1332), [abi.rs:265](host/apple/src/abi.rs:265): **Refusal still occurs after some native dispatch paths.** An in-process Rust source returning `Request::native(payload).long_poll(...)` invokes its native handler: admission omits the check, and handoff bypasses `execute`. Auth/surface requests similarly bypass it in Apple/Linux presenters. The seam rejects these, but directly linked sources do not. Validate before dispatch and executor admission.

**Should-fix:** None additional.

**Nits**

- [executor_longpoll.rs:433](host/apple/src/executor_longpoll.rs:433), [http-body.js:155](host/web/http-body.js:155): IMF-date parity remains incomplete. Native accepts signed times such as `-1:00:00`; web rejects them. Web normalizes out-of-range fields through `Date.UTC`; native rejects some. Waits remain bounded.

The marker isolation, revised folded-reply counting, and ABI 4 wiring look consistent by inspection. All four permitted test commands stopped before assertions because the read-only sandbox denied Cargo writes. No files changed.
