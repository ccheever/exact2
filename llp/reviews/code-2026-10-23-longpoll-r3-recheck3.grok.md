# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `619c082be6`.
- **Method:** one brief (sha256 `d865d654bad0c3700822d1e195fbf96658a69ec811718bf07f43f84c986af13e`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking.** The terminal host still runs a bad long poll's continuation. Apple, Linux, the renderer, and page enumeration all enter through `admit`, and an ordered refusal there fences later work. A request that is not a long poll is not refused by this check. The wasm batch refusal is unchanged. `cargo test -p exact-apple --lib long_poll`: 13 passed, 2 ignored.

- `host/terminal/src/requests.rs:64`. `run_requests` never calls `long_poll_refusal` or `admit`. A continuation is `dispatch_work`'d and, at `:81` and `:84`, run (`Now` on a thread, `Later` inline), including one `take_requests` just armed. A non-continuation is `refuse_request`'d at `:94` with the generic terminal reason and no ordered barrier, so a later ordered continuation in that same batch still runs. `llp` and `harness` pass their sources straight into this host. Page enumeration does go through `admit` (`host/render/src/pages.rs:68`).

**Should-fix.** None.

**Nits.**

- `host/apple/src/executor_longpoll_tests.rs:603`. `Request::native(...).long_poll(...)` is independent (`runner/src/request.rs:255`, `is_ordered` at `:309`), so `admit` does not raise `ordered_barrier` (`host/apple/src/executor_core.rs:551`). The following ordered GET can leave. The script records a URL only after asserting `Accept: application/json` (`executor_longpoll_tests.rs:44`), which that GET does not send, and the worker catches the panic (`executor_core.rs:1243`), so `asked` stays empty and this iteration cannot fail. Storage and continuation in the same test are ordered and do stay fenced.

Astra's consumed-refusal rollback is the generic admission path (`pending.refused = ordered` in `runner/src/runner/admission.rs:119`), deadlines included. `QUEUE.md:3` already has it.
