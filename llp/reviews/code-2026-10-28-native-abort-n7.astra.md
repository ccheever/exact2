# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 7, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `39be9c166f`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** The change it reviewed (the executor's ended reports) was reverted after this review; the candidate returned to b4cc675603.

The blocker is **not closed**. Read-only source review; no builds or tests run.

1. **Completed-but-undrained requests still leave stale records.** If completion wins the race with forgetting, `complete` removes the running entry and queues its outcome without reporting `ended` ([executor_core.rs:949](host/apple/src/executor_core.rs:949)). A subsequent superseding input makes `forget_with` discard that outcome, again without reporting its ticket ([executor_forget.rs:62](host/apple/src/executor_forget.rs:62)). No later completion can repair this. The async machine has the same omission when forgetting follows terminal settlement.

   Requests dropped from `fenced` also disappear without an end report ([executor_forget.rs:111](host/apple/src/executor_forget.rs:111)). These paths can leave retired records after their requests have ended or been dropped unsent. Enough such records still evict the older, live request at the [256-record cap](runner/src/runner/admission.rs:164). Thus the acknowledged Placed race can still expose the original blocker.

2. **`ended` can grow without bound when input continues without pumping.** It is an unrestricted vector, emptied only by [`take_ended`](host/apple/src/executor_forget.rs:31). Both [queued-job removal](host/apple/src/executor_forget.rs:50) and [forgotten completion](host/apple/src/executor_core.rs:954) append a ticket while releasing admission counts and bytes. Those released slots can therefore be reused indefinitely without consuming the notifications.

   This is reachable through host input: Apple dispatch calls `emit`, which forgets old requests and admits replacements independently of `pump` ([abi.rs:1049](host/apple/src/abi.rs:1049), [abi.rs:256](host/apple/src/abi.rs:256)). Moreover, a forgotten independent completion does **not** wake the host; the completion branch wakes only when the ordered lane becomes empty ([executor_core.rs:959](host/apple/src/executor_core.rs:959)). Repeated supersession of independent reads can therefore accumulate notifications without a completion wake. With no further submissions, accumulation is finite.

3. **New: pooled render executors can report another runner’s ticket ended—a harmful false end.** Render completion checks the runner’s pending tickets, which exclude superseded requests ([lib.rs:823](host/render/src/lib.rs:823)). The executor is then forgotten and pooled without waiting for forgotten writes to finish ([lib.rs:491](host/render/src/lib.rs:491)). The next render reuses that executor with a fresh runner whose tickets restart at `1` ([lib.rs:436](host/render/src/lib.rs:436), [runner.rs:865](runner/src/runner.rs:865)).

   Consequently, old render A’s forgotten request `k` can finish after render B has issued and retired its own request `k`. The new [unqualified delivery of `ended`](host/render/src/requests.rs:44) removes B’s abort record even though B’s request still runs. End reports need isolation between runner lifetimes.

For the remaining lifetime cases:

- **Covered:** ordinary queued reads/app-cancelled jobs, and runs forgotten before completion. Streams report through `complete` when their reader ends—for example [executor_stream.rs:180](host/apple/src/executor_stream.rs:180). Dropping an intermediate stream message does not falsely report an end. Already-completed stream outcomes share issue 1.
- **Async physical lifetime:** [`settle` reports `ended`](host/apple/src/executor_async.rs:1098) before checking whether physical work remains; [`physical`](host/apple/src/executor_async.rs:1135) explicitly permits `live > 0`. Cancellation and timeout can therefore report before transport teardown. Their cancellation path already owns the abort, so that timing alone is not an additional lost-abort blocker.
- **Core shutdown:** retirement does not report every ticket, and subsequent completions return immediately ([executor_core.rs:768](host/apple/src/executor_core.rs:768), [executor_core.rs:943](host/apple/src/executor_core.rs:943)). Ordinary native replacement also replaces the runner ([abi.rs:924](host/apple/src/abi.rs:924)); I found no separate surviving-runner problem there.

Verdict: blockers — missing end reports, unbounded `ended` accumulation, and false ends across pooled render runners.
