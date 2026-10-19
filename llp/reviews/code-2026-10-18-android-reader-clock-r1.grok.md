# Code review: the Android reader's driver launch on the runner's clock from its boot, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f9cda636e`.
- **Method:** one brief (sha256 `911e261e04927e6891e84931070f95fc8c1b7504f874e93e7afc41e004d105ac`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; nothing else samples the wall into the runner while the seek flag is set.
- **Disposition:** nothing to take.

Nothing is blocking. The agent's first frame still paints at runner time 0, and `ready` still follows that draw.

1. **First content and `ready` do not wait on the clock.** `CanvasHost::boot` still builds a dirty tree, and `startAsync` calls `frame()` immediately. With `SEEK` set, `now()` is the runner clock (`host/linux/src/canvas.rs:1188`), which `Launch.clock = None` leaves at 0 (`host/linux/src/app.rs:330`). `frame_held` paints that tree before any timer is due. `ExactView.onDraw` posts `onFirst` as soon as the stream is non-null (`ExactView.kt:460`), and `AgentDrive` sends `ready` on that latch (`AgentDrive.kt:45-46`). Data, images, and fonts are not clock gates: data and images wake on fds (`poll` / `pump` / `poll_images`), fonts are measured at layout, and GPU canvases are deferred until after first content (`canvas.rs:1262-1265`). Hatch `after` and timers with a positive delay stay unfired until a `clock` request, which is the point of the change. They are not what produces the first stream. Frame tasks and `tick` run at that same 0. They do not stall the paint.

2. **Nothing else samples the wall into the runner while `SEEK` is set.** Touches, scroll lead/settle, and `gpu_render` all take `CanvasHost::now()` (`canvas/input.rs:56`, `canvas/scroll.rs:57`, `canvas/refine.rs:135`, `surfaces/android.rs:466`). The reader's fling and the 4 ms GPU-device retry are wall delays around the host. They do not write a wall reading into the runner. The supported drive passes `lower=0` (`scripts/agent-android.mjs:189`), so layer tracks are sampled by the engine on this clock, not by the reader's monotonic sampler.

3. **The date matches a zero clock.** `EXACT_AGENT=1` makes `agent_time` the epoch (`zone.rs:32-40`) and `config.agent` drops `clock_started`, so `performanceNow()` is 0 and the date is `EXACT_AGENT_EPOCH` plus 0.

4. **Other launches are unchanged.** The filter is `clock_started.filter(|_| !config.agent)` (`app.rs:330`). A non-agent canvas boot still gets `wall_since(started)` (`canvas/agent.rs:75`). The Linux display and the stdio agent never went through `own_time`.

The new test checks the boot half only: an agent `Config` arms `after(10000)` at 10000 (`presenter/painter.rs:274-287`). It does not set `EXACT_AGENT`, so it does not cover `SEEK` or the epoch.
