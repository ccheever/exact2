# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `420715994`.
- **Method:** one brief (sha256 `12752839cf4c0f9c0cf96a48310391dbdb5938922a39389ac4ca79d76d273819`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking in the web change; one should-fix (Linux, Android and the canvas host start their clock before the config loads, which can fetch a plan, so a slow start left boot timers behind) and a nit (the web's fresh boot_plan path untested).
- **Disposition:** both taken in `2a27c7139` (Config::clock_started; the boot starts at the display clock's reading and the date at its zero; a boot_plan test).

Nothing blocking in HEAD’s web change.

1. **SHOULD-FIX — The Linux/Android claim overlooks delayed startup (pre-existing).**  
   [app.rs:527](/tmp/rv-ios-5/host/linux/src/app.rs:527) records `started` before configuration loading, which [synchronously fetches a URL](/tmp/rv-ios-5/host/linux/src/app.rs:164). [android.rs:426](/tmp/rv-ios-5/host/linux/src/android.rs:426) and [canvas.rs:1076](/tmp/rv-ios-5/host/linux/src/canvas.rs:1076) have the same ordering. Their fresh runner still receives [clock: None](/tmp/rv-ios-5/host/linux/src/app.rs:334).

   **Scenario:** fetching the plan takes 15 seconds; a mount `after(10000, …)` boots with deadline 10000 and fires on the first live advance, instead of ten seconds after boot.  
   **Fix:** pass the current live-clock reading into each fresh native boot, with `epoch_at_zero` using that same origin. Qualify the commit message meanwhile.

2. **NIT — The fresh `boot_plan` branch lacks regression coverage.**  
   [clock_tests.rs:62](/tmp/rv-ios-5/host/web/src/abi/clock_tests.rs:62) tests fresh `boot`; the existing `boot_plan` test already has a host and therefore carries its clock.

   **Scenario:** an initial dev generation enters `boot_plan` without an existing host. Removing HEAD’s change at that call site would leave these tests passing.  
   **Fix:** run the fresh-boot assertion through `boot_plan` on an empty bridge too.

The web wiring is correct by inspection: [glue.js:1305](/tmp/rv-ios-5/host/web/glue.js:1305) enables the clock after asynchronous preparation and before either boot. Its import reads `performance.now() - t0`, matching subsequent advances. Fresh seeds use that reading; carried and checkpoint seeds retain their clocks. Valid rendered checkpoints pass through `without_launch_time`; invalid or wrong-location checkpoints fall back fresh outside that wrapper. Fresh dev/page boots are covered by the implementation.

Linux reloads carry state; Android render-thread restarts create a new origin; Canvas activity recreation adopts the existing host. Those lifecycle paths do not expose another late fresh boot against an old origin.

No builds or tests run; no files or index changed.
