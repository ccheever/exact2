# Code review: launch facts round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `70cde61f48bb69c54fea1aef46938d000c63a72c91a1280b1a8de9978aa4d4cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR (every checkpoint dropped the date) taken in round 3's code; Grok's (the Linux loops' clock zero) pre-existing in kind, recorded in QUEUE.

---

## MAJOR

**host/web/src/abi.rs:293** — Any checkpoint string strips the launch date before the boot decides which clock it is on.

```292:295:host/web/src/abi.rs
        let launch_facts = exact_runner::time::Launch {
            time: self.launch_facts.time.filter(|_| checkpoint.is_none()),
            place: self.launch_facts.place.clone(),
        };
```

`checkpoint.is_some()` is true as soon as `exact_checkpoint` was called. Three boots inside that `launching_with` start at clock 0, so the date they need is the page's or the drive's epoch:

- **host/web/src/page.rs:477** — An agent digest ending in ` driven` (glue.js:1320) forces `now_ms` to 0. The drive epoch is already gone, so the first settle reads epoch 0 and offset 0. `followOffset(now())` at glue.js:1359 then `set_time`s the drive epoch and recommits.
- **host/web/src/page.rs:482** — A checkpoint that does not parse calls `Host::boot_linked` fresh (`now_ms` 0) under that same stripped date.
- **host/web/src/page.rs:492** — A checkpoint rendered at another path does the same.

A successful non-agent adoption keeps the renderer's Unix-millisecond `now_ms` (page.rs:481). With the date dropped, that first settle is `date = now_ms`. The filter belongs only on that adoption. The place is still applied on every checkpoint boot. `the_launch_place_and_date_are_the_first_batchs` (host/web/tests/it/page.rs:352) boots with no checkpoint.

**host/linux/src/app.rs:321** — The one sample is still before the runner's clock exists.

The comment at app.rs:318 says this sample is the clock's zero. `epoch_at_zero` is taken at app.rs:321, then `Module::resolve` / `replacement` (app.rs:378), then inside `Presenter::boot_selected` the font engine and `painter.open` (presenter.rs:382), and only then `Host::boot_decoded` (presenter.rs:388) builds the runner at `now_ms` 0 and copies `launch_time()`. The first settle's date is the wall time from before that work.

`delivered` tells that same epoch (app.rs:349). `set_time` treats an equal epoch as the same fact (runner/src/runner/time.rs:61), so a boot of a second or more stays early. Headless (app.rs:612) calls `frame()` and `first_pixel()` without advancing, so that early epoch is the process's date. An agent boot uses `agent_time` and is unaffected.

The display loop's zero is a different instant: `started` at app.rs:526, before `Display::open`, and `wall` at display.rs:339 is `started.elapsed()`. The first advance dates `epoch_at_zero + time_since_process_start`, ahead by everything before the sample. Telling this one sample both ways does stop a later epoch from being recommitted on top of that elapsed time. The first settle, and a headless run, still freeze the pre-boot sample.

## Checked

The launch thread-local restores the outer `Launch` on unwind, including a nested `launching_with`. `launching()` installs `place: None` for its own boot; production boots call `launching_with`.

The locale slot is written in runner/src/runner/router.rs:581, immediately after that slot's own initializer and before every other one, with `resolve_locale` — the same write as `set_place` (runner/src/runner/time.rs:109). A plan with no tables resolves to `None` (plan/src/strings.rs:132) and the write is skipped. A carried or persisted value is replaced, as `set_place` replaces it. The first batch's `language` op reads that slot; the wasm test's root initializer `seen` reads `ar`.

These boots enter `launching_with` on the thread that constructs the runner: both `Presenter::boot_selected` calls in `boot_presenter_with_painter`, wasm `boot` (covering `boot_checkpoint_linked`) and `boot_plan` (covering `boot_module`), and both Apple `Host::boot_stored_after_decode` calls (host/apple/src/abi.rs:540 and :842) on the bridge owner thread. QUEUE already names the presenter's reload, the render host, the content-region host, and a direct `Host::boot_stored` as still outside.

`Place::launched` copies the place out of the input buffer before returning. Glue's `launchLocation()` bytes are a separate array; `exact_set_launch`'s `writeIn` at glue.js:1305 finishes before the checkpoint write and the boot `exact_in`, including across the `loadStage` await. Apple's seed is a `UInt32` (Session.swift:35), and the web and Linux seeds are 32-bit, under `SEED_LIMIT`. Agent clocks stay on the drive epoch: web `timeReporter` / `placeReporter`, Linux `agent_time` / `launch_place`, Apple `dateNow` when `launchPlace.epoch` is set.

glue.js and host/apple/src/abi.rs are 1499 lines. host/web/src/abi.rs is 1486.

NOT READY
