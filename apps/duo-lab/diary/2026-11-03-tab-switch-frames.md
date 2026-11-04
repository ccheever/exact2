surfaces: ios
agent: Claude Code (Claude Opus 5.5)
task: Find why a tab switch kept the app drawing with no touch, and fix it.

### Rough

On an iPhone Duo at 120 Hz, switching to Reflow and touching nothing made about 1.5 s of updates, some of them late, as the frame health card's agent report showed. Finding their source took about three hours. `agent.mjs` switches tabs through the host's activation and cannot touch UIKit's tab bar, and its touch runner cannot tap the tabs at all. `sample` read no stacks of the simulator's process, `xctrace record --launch` failed, and `xctrace record --attach` waited for hours after the driven app closed. What worked was a local build whose `start_url` was `/reflow`, with `NSLog` around `FrameClock.want` and `drop` and the scroll pump's passes, read with `simctl spawn … log stream`.

Two things asked for those updates. One was in exact2: at rest, the scroll pump kept the display link at full rate while every text worker's slot was taken, and now waits for a slot to free without frames. The other was UIKit's own, most likely the tab bar's selection animation: once the card's report named who asked for each late frame (#1377), none of Exact's frame-clock users had asked for those. The report's work figures also counted the main thread's waits within an update, which #1377 now leaves out.

A launch location for native apps would have helped: `agent.mjs` opens the web at one (`--url`), while on iOS `type <root> /reflow` navigates only once the app has launched.

### Lean in

The card's report on the phone told back-to-back updates apart from a single stall, which pointed at a loop. Exact's own signposts (`pump-text`, `text-raster-worker`) streamed with `log stream --signpost`.

### Needed

none

### Checkpoints

- iOS build: smooth.
- driving the app: rough (see Rough).
