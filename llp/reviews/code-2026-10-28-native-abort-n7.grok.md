# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 7, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `39be9c166f`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** The change it reviewed (the executor's ended reports) was reverted after this review; the candidate returned to b4cc675603.

The blocker is not closed. A let-go request whose reply is already sitting in `completed`, or that was dropped while fenced or waiting, never gets into `ended`, so that record still lives until the 256 cap.

**(a) Reporting**

These ends are reported:

- A forgotten sync run, after the worker removes it from `running` (`host/apple/src/executor_core.rs:949-955`).
- The async machine's terminal, when the run is already forgotten (`host/apple/src/executor_async.rs:1098-1102`).
- A queued safe read, or an app cancel, dropped in `forget_with` (`host/apple/src/executor_forget.rs:50-56`).
- A stream, listener, long poll, or socket that is still in `running` when let go: the reader then calls `complete`, which pushes (`host/apple/src/executor_stream.rs:379`, `executor_listen.rs:272`, `executor_longpoll.rs:168`).

These ends are not:

- A completed, undrained outcome. `forget_with` drops it and does not push (`host/apple/src/executor_forget.rs:62-81`). That is the usual race. `drain` takes one outcome per pump (`executor_core.rs:622-667`); `forget` runs after the commit (`host/apple/src/abi.rs:256-258`, `host/linux/src/presenter.rs:811-814`). A commit that supersedes another target (`runner/src/runner/commit.rs:858-860`) retires a ticket whose reply is already in `completed`. The reply is dropped, `fulfill` never runs, and `fulfill` is what would have removed the record (`commit.rs:1186-1187`). The same drop covers a re-ask marker already placed (`executor_core.rs:869-879`) and a stream whose final outcome is already queued.
- A fenced request let go unsent (`executor_forget.rs:109-119`). No push. A queued safe read on the same path does push.
- A re-ask still in `waiting` (`executor_forget.rs:106-108`). Dropped, never settled, no push.
- Retirement. `Core::retire` clears `completed` and `waiting` (`executor_core.rs:768-781`). `complete` returns once `retired` is set (`executor_core.rs:943-945`). The async disposer drops queued jobs and fenced work (`executor_async.rs:881-891`). On Apple and Linux the runner drops with the core. On a render deadline the executor is dropped and the runner is kept (`host/render/src/lib.rs:493-494`, `501-505`), so those records stay in `retired_fetches`.

A push happens only when the outcome is decided: the worker has left `running`, the queued job has been removed, or the async machine has taken the terminal transition. That is the same moment a held request's reply is published. `settle` can run while `live > 0` (`executor_async.rs:1098-1118`); for a cancel, that terminal is the `Cancelled` outcome posted from `forget_with` (`executor_async.rs:393-395`), and the exchange abort is signaled before `forget_with` returns (`executor_forget.rs:134`). The runner applies the ticket at the next pump.

`requests_ended` runs at the start of the Apple pump (`host/apple/src/abi/pump.rs:13-14`), the Linux pump (`host/linux/src/presenter/pump.rs:18-20`), and render's `settle_aborts` (`host/render/src/requests.rs:44`), before `collect_aborts`. The new tests cover a queued read and a forgotten write (`executor_abort_tests.rs:173-191`) and a runner removal (`abort_tests.rs:148-169`). They do not cover a completed outcome, a fenced request, or a waiting re-ask.

**(b) `ended` is uncapped**

`ended` is a `Vec<u64>` (`executor_core.rs:137-139`) drained only by `take_ended` (`executor_forget.rs:31-33`). A forgotten completion releases its admission count in the same step (`executor_core.rs:954-958`), so the in-flight window does not cap the vec. A host that keeps committing and never pumps grows it without bound.

Apple pumps only from the executor wake (`host/apple/Sources/ExactKit/Session.swift:573-578` → `pumpReplies`). A forgotten completion wakes the host only when the ordered lane goes idle (`executor_core.rs:961-963`; same condition in `executor_async.rs:1119-1121` and `executor_forget.rs:124-126`). Superseded independent fetches push `ended` and do not wake, so both the vec and `retired_fetches` keep growing until some other reply wakes a pump. Linux drains it every display-loop turn (`host/linux/src/display.rs:525`).

**(c) Render pool**

A settled render does `forget(|_| false)`, drains once, and returns the executor to the pool (`host/render/src/lib.rs:493-498`). It does not take `ended`. The next page's runner starts tickets at 1 (`runner/src/runner.rs:865`) and its first `settle_aborts` applies that leftover vec (`lib.rs:795-799`, `requests.rs:44`). A late completion from the pooled core pushes into the same vec. A ticket that ended on the previous page removes the next page's let-go record while that request still runs.

Verdict: blockers — completed, fenced, and waiting let-go ends are never reported, so the 256 cap still evicts a live record; `ended` is uncapped when a host does not pump; a pooled render executor applies the previous page's tickets to the next runner.
