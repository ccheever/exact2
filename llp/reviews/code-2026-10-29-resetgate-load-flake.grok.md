# Code review: ResetGateTests admission check load flake, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `a7d20649ab`.
- **Method:** one brief (sha256 `c305148c36b4c13f3fb710a89efce7641a643d0a7a4501a355eaa55920ddadb8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The minute cutoff still admits the task before any Foundation timer fires, and the three claims stay non-vacuous.

`every(60000, …, wait=update)` is not a native task timer (`wait != 0`). Its nominal due is the boot launch clock plus 60s. `TimerAdmissions` only uses a Foundation timer as a wake at that due (`TimerAdmissions.swift:111`). `check(update:at:)` does not wait for it. The cutoff is passed into `release_waits`, which runs a wait whose `next_ms <= cutoff` (`waits.rs:146`). At the call, `session.now()` is already past the arm time, so `now + 60_100` is past `arm + 60_000`. The check fires `tick` on that call. The final `until` is 5s, and the wake is still about a minute out, so `count() == 1` cannot come from that timer.

That is why the hold checks still mean something:

- The held `check` is a real firing attempt. `held()` records `timerWaits` / `timerWaitsAsync` and asserts the runtime was not entered. If the hold failed open, `count` would become 1.
- `hasPendingForTests == false` is the async path setting `pending` and then clearing it when the held batch lands. The interval does not skip that.
- After `apply`, the new generation arms again at its own launch clock plus 60s. The second `check(update:at: now + 60_100)` is what produces `count() == 1`. A real update callback in this test calls `check(update:)` with cutoff `now`, which is still short of the due, so it cannot admit the task first.

The old race remains only if the arm-to-hold gap exceeds a minute. `until` gives the boot 5s, so a loaded boot cannot cross it.
