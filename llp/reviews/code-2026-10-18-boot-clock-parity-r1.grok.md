# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `420715994`.
- **Method:** one brief (sha256 `12752839cf4c0f9c0cf96a48310391dbdb5938922a39389ac4ca79d76d273819`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking, no findings.
- **Disposition:** nothing to take (its reading of Linux's startup gap as the launch's own startup was overtaken by Astra's should-fix, taken).

Nothing is blocking. No findings.

HEAD (`4207159940`) samples the page clock inside `Bridge::launch()` and passes that `Launch` into both boots. `Seed::Fresh` is the only seed that reads it (`runner/src/runner.rs:729`). Checkpoint and carried seeds keep their own `now_ms`.

**Web.** `glue.js:1305` calls `exact_live_clock` after the font and shader awaits and immediately before `exact_boot` / `exact_boot_plan` / `exact_boot_module` (`glue.js:1306–1321`). The glue installs the sampler; Rust reads it at `host/web/src/abi/clock.rs:18–22` as the boot starts, so the sample includes that pre-boot wait and matches later `performance.now() - t0` advances. Agent mode passes `0`, so the sampler stays unset and a fresh boot stays at 0. `exact_set_launch` at `glue.js:1303` writes the date and place first; `launch()` only overlays `clock`.

A rendered page goes through `exact_boot` with `page.checkpoint`. `host/web/src/page.rs:521–526` runs that boot under `without_launch_time` (`runner/src/time.rs:111–118`), which clears the sampled clock, and `Seed::Checkpoint` starts at `checkpoint.now_ms` (`runner/src/runner.rs:732`). A drive rewrites the digest with ` driven` (`glue.js:1319`) and forces `now_ms` 0 (`page.rs:477–479`). An unreadable checkpoint or a location mismatch falls through to `Host::boot_linked` (`page.rs:483–495`) while still inside the outer `launching_with` (`abi.rs:296`), so that fallback fresh boot does start at the page reading.

`boot_plan` (`abi.rs:333–352`) passes `Host::carry()`. In-place dev reload and module replacement take that path, so they keep `carried.now_ms`. The `fresh` flag only skips focus restore (`glue.js:1304`). A full reload (`f`+Enter, a program change, relaunch) is `location.reload()`, which is a new document and a new `t0`. The first boot with a dev plan and no host is `Seed::Fresh`, and the sample applies. Harnesses that boot without `exact_live_clock` stay at 0. The new test (`clock_tests.rs:62–82`) asserts the ticker is due at 61000 after a boot at 60000, and at 1000 after the agent clock takes over. The missed-beats test sets the page to 500 before boot so those beats are not consumed by the new start time.

**Linux and Android.** The claim holds. Both boot with `Launch.clock: None` (`host/linux/src/app.rs:328–335`) and install `wall_since(started)` only after that boot. `started` is the origin of this launch: `app.rs:527` for the display session, `android.rs:426` at the start of the render thread, `canvas.rs:1076` at the start of `CanvasHost::boot`. Dev reload and bundle activation pass `Some(&carried)` (`presenter.rs:562–578`). Activity recreation adopts the running host (`canvas/jni.rs:130–135` and `191–197`; `canvas/reader.rs:43` boots nothing). Surface attach and detach do not boot a runner. No second fresh boot on an already-advanced wall showed up.

`started` for the Linux display session is taken before config and DRM open, and fonts load before the runner is armed, so the first frame’s sample is that startup later than 0. That is this launch’s own startup, with the runner armed at the clock’s origin. It is the case the message treats as already matching.
