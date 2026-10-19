# Code review: a fresh boot on a live clock starts the runner's clock at its reading (StoreRefreshTests), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `62b1d3eef`.
- **Method:** one brief (sha256 `1f7bb25943eadf02110fb25becd7a8b7e118901387bb2af89c2b45fb64b72175`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking, no findings.
- **Disposition:** nothing to take.

Nothing is blocking. No findings.

The fresh-session fix is wired the way it was approved. A `Seed::Fresh` boot starts at `launch_clock()` (`runner/src/runner.rs:729`). `launching_with` keeps a clock only when it is inside `0..=MAX_CLOCK_MS` (`runner/src/time.rs:100`); anything else is dropped and the boot starts at 0. `Carried`, `Checkpoint`, and `At` keep the seed’s own `now_ms`. Apple samples the presenter’s wall into that fact on both window paths (`host/apple/src/abi.rs:526` and `:838`, via `Bridge::launch` at `host/apple/src/abi/preferences.rs:170`). An agent boot still starts at 0: `session.clock = 0` is set before `primeTime` whenever `agentFreezes` (`ExactMac/main.swift:179` and the iOS/host twins), so `live_clock` is `None`.

Timers, one-shot `after`, frame tasks, gate bases, and `transport_stood` arm from `runner.now_ms` after settle (`runner/src/runner.rs:1050`). A boot commit’s `start_commit` sets `clock.started` to that same value, so a `then` or queue `next` armed by the boot is due at the launch sample; a zero-delay `then` still runs on the next advance. While the host is not presenting, a frame task’s virtual frame (`now + 1000/60`) is inside `timer_due`; a window session’s display link calls `frame`, and `advanceIfDue` skips when frame tasks exist. `performanceNow()` on the first settle is the launch sample. `primeTime` sets `epochAtZero` to `Date − now()`, so the date is the real one. A kept answer whose identity baked `performanceNow()` now differs across process launches, which is the same date the launch facts were added to carry. Checkpoints and carries keep their stored `now_ms`, so a reload does not jump to a new wall sample. Linux, web, and the terminal pass no clock.

`Host.now_ms` is still 0 and the engine is still created at 0 (`host/apple/src/host.rs:518`, `:549`) while the runner may already be at the wall. `engine_time` clamps a commit to the host clock (`host/apple/src/first_frame.rs:101`). Production calls `startOnFrame(true, at: 0)`, and pending plays stay frozen at their begin until `present` calls `start_frame` (`host/apple/src/presence.rs:315`), so the engine’s first seek from 0 to the wall does not elapse them. Sounds and MIDI map runner milliseconds as `t0 + at/1000`, so a play scheduled at `performanceNow()` during boot lands at process-now.

Leaving the clock off `boot_entry` is right. `host/apple/src/abi/snapshot.rs:36` (and `refresh_entry`) still boots from `launch_facts`, whose clock is unset. Widget and activity extension sessions prime `SessionFacts` with clock 0, so `performanceNow() == 0` is the entry date, and `snapshot_timeline` uses the unix date only as the refresh argument. Sampling `ExactEnv.wall()` there would shift that date by process uptime. The in-app gallery is a different domain and is
