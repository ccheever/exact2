# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `1bdeb192fb`.
- **Method:** one brief (sha256 `feffed37b58c1c866c2eda820c220c8e90c37279830af26539aa7eb7f37ddc37`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- `runner/src/runner/commit.rs:820`, `host/apple/src/abi.rs:263`, `host/apple/Sources/ExactKit/Session.swift:568`: The runner records the refusal and leaves it out of `take_requests`, but Apple only delivers an admission refusal from `pump`, and `pump` runs when the executor wakes. This refusal never enters the executor, and `emit` does not call `notify` (an auth answer that settles in the runner does, at `host/apple/src/abi/commands.rs:191`). `PendingReq::in_flight` stays true while `refusal` is set, and `take_request_refusal` does not drop the row. On an interactive Apple session the resource stays pending and the source never receives the failure. The same hang hits any refused long poll, including a bad HTTP one that previously failed inside `execute` and woke the pump by completing. Linux still settles: the display loop and `wait_for_replies` call `pump` on their own. The web fulfills inside `complete`. The renderer puts the ticket in `held.refused`, so `settle` returns.

**Should-fix**

- `runner/src/runner/commit.rs:820`: The refusal is recorded with `ordered: false`. A storage, continuation, or ordered HTTP long poll is ordered (`Request::is_ordered`). Host admission refusals pass that flag so `has_ordered_request_refusals` holds `resume_ordered` and `take_request_refusal` waits until `ordered_idle`. This one can commit the failure while earlier ordered work is still in flight. Pass `request.is_ordered()`.

**Nits**

- `runner/src/runner/stream_tests.rs:329`: The test stops at `take_request_refusal` and never `fulfill`s, so the ticket is still pending when it ends. It only builds a native long poll.
- `runner/src/runner/commit.rs:819`: A continuation long poll is refused without `discard_request`. On a placed source, `parse` still consumes `Stage::Turn` once the refusal is fulfilled, but the `recorded` job stays: `fulfill` does not set `forgot`, so `forgotten` does not drop it. Dropping the stage before that `parse` turns the refusal into "a reply for an answer not in flight".

`take_admission_refusal` only reads `pending.refusal` and the background refusal. It does not call `take_any_settled` and does not touch `auth.settled`. The web still delivers an auth answer with fulfill kind 9 (`take_settled` at `host/web/src/host.rs:1263`). Apple and Linux still prefer `take_any_settled` inside `take_request_refusal`. A long poll on an auth request is refused before `auth::arm`, so no session is opened. A request with no long poll still has `long_poll_refusal() == None` and is queued as before. The `Retry-After` clock check and the JS seam's bad `same` byte match the round-3 nit. Tests were not run.
