# Code review: the Android reader's driver launch on the runner's clock from its boot, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `529890789`.
- **Method:** one brief (sha256 `3b1496de040d72c6d5e3ed86c6b5e88aafa46c916e5e98cc160c11abc247cc6d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking.
- **Disposition:** nothing to take.

Nothing is blocking. The round-1 skip holds: an Android reader agent launch never presents a runner frame, virtual frames still run on the driver's clock steps, and a non-agent launch still presents on the display.

`present_frames` stays off for the whole reader launch. `Runner`'s flag defaults to false, and the only write of `true` in this host is `Host::frame`, which only `Presenter::animation_frame` calls. The production callers of that are the canvas turn, the Linux display loop, and the Surface host loop.

- The reader is the canvas host. `own_time` sets `SEEK` when `EXACT_AGENT=1` before the host is returned, and `startAsync` paints only after that. `agent::clock` is therefore `Some` from the first turn through every later one (`DRIVEN` as well, once a request arrives). `frame_held` skips `animation_frame` for that whole life, so nothing on this launch calls `Host::frame`.
- The Linux display loop is not entered for an agent config. `Config::headless` is true when `config.agent` is set, and `run_config` serves stdio instead. That path never calls `animation_frame` either.
- `android::start`'s loop still calls `animation_frame` and still installs the wall clock. That is the Surface host, not the reader, and it is not the seek-clock launch.

Virtual frames fire on the driver's advances. With `presenting` left false, `advance_within` treats a frame task as a timer (`frames = !self.clock.presenting`) and schedules it at `base + k·1000/60`. A driver `clock` goes through `clock_hatched` → `clock_stepped` → `Presenter::clock` / `clock_until_request` → `Host::advance` / `advance_until_request`, all of which are that advance. The first virtual frame is `k = 1` (about 16.7 ms from boot), not time 0, so the pre-`ready` turns at the unmoved runner clock do not fire it. A display turn calls `advance` only when something is already due at that same runner time, which a completed driver advance has already consumed.

Nothing else that has to happen before `ready` goes through a presented frame:

- First paint still happens. The presenter boots `dirty`, and `display_frame` paints through `Presenter::frame`. `AgentDrive` waits for that content before it sends `ready`. That paint is not `Host::frame`.
- Surfaces still get their turn. GPU canvases render from `frame_held` (`render_surfaces`) once the first paint has led, at the runner's `now`. A 2D canvas gets its frame signal from that same paint (`sync_canvases(..., true)` → `canvas_frame`), once per clock value, which is one draw at 0 before `ready` and one at each later landing time.
- Hatch frame tickets no longer tick on display refreshes. `hatch_presented` runs only from `animation_frame`. Under the seek clock they tick in `hatch_fire`, which `clock_hatched` calls at each hatch instant on the way to the driver's target. Before `ready` there is no seek, so a ticket taken from a `built` moment waits, as it does on the stdio agent. `after`s that are already due at the current runner time still fire: the turn's `advance` calls `hatch_timers`.
- Motion still ticks. `frame_held` calls `Presenter::tick` whenever `needs_animation_frame()` is true, including while a frame task or a hatch ticket wants frames. At the unmoved boot time that seek does not move the engine. A driver `clock` seeks it in `clocked`. Lowered layers still ship in the paint stream.

Non-agent launches are unchanged. With `EXACT_AGENT` unset, `SEEK` stays false, `own_time` still installs `wall_since(started)`, and `agent::clock` is `None`, so the canvas condition is the old `wants_frames()` check. The display and Surface loops were not edited. `clock_started` is dropped only when `config.agent` is set; the boot test still expects a non-agent `after(10000)` to arm from the display reading and an agent config to arm at 10000.
