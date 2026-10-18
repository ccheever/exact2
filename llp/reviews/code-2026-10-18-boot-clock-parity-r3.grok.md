# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `530ed93a3`.
- **Method:** one brief (sha256 `f09307dfb53af57646f16225afbf66d07920a4900959fb49415bb3f8b3db18de`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** the same should-fix as Astra's (round 1's boot at the display reading did not fire those timers); the fallback epoch and Windows hold.
- **Disposition:** as Astra's: reverted in `02fd8a42c`, the reader's seek clock QUEUEd.

Nothing blocking. The stdio driver, the baked fallback, and the Windows window boot all do what round 2 asked. One follow-on on the Android reader.

1. **SHOULD-FIX** — `host/linux/src/app.rs:329` with `host/linux/src/canvas.rs:1122` and `host/linux/src/canvas.rs:1231`. **Scenario:** An Android reader drive is a real `config.agent` launch. `ExactActivity` writes `x_agent=1` into `EXACT_AGENT` before the library loads, and `Config::from_env` snapshots that into `config.agent`, so the new filter is the right signal and the boot really does arm at zero. The stdio agent stays at zero too: it never sets `clock_started`, and `headless` never installs a wall clock. The reader then always calls `set_live_clock(wall_since(started))`, and `AgentDrive` does not send `ready` until the first content frame. That frame's `now()` is still the wall (`canvas.rs:1189`), because `DRIVEN` flips only inside the first agent request (`canvas/agent.rs:92`). `animation_frame` / `advance` therefore move the runner from 0 up to time-since-`started` (that `Instant` is taken before `from_env`, so a dev-plan fetch is included). A seek cannot go backward, so the driver inherits that reading, and every timer due in the gap has already fired. Round 1 had armed those timers at the display reading, so the same frame did not run them. **Fix:** On a reader drive, do not install the wall clock, and feed the runner's own `now` (0) into pre-`ready` frames. Skipping only `set_live_clock` is not enough: `Runner::frame` still advances to the `now` argument.

Checked and holding:

- **Fallback epoch.** `epoch_at_zero` is computed once as wall minus the display reading (the date at that `Instant`'s zero) and stored on `launch_facts`. Each attempt only overwrites `clock`. The second attempt's date is the same epoch plus a fresh reading, so it stays lined up with the display clock. Under the agent, `agent_time` supplies the epoch and both attempts pass `clock: None`.
- **Windows.** `host/windows/src/window.rs:402` sets `clock_started` from `self.started` before `boot_presenter`, same as `display.rs`. `config.agent` already returned through `run_config` / `headless`, so this line is display-only.
