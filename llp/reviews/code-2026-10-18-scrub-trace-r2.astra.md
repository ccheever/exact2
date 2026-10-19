# Code review: the Linux and Android scrub trace, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the trace commit and round 1.
- **Method:** one brief (sha256 `ab10f2b1fa09983dd3361b9a0da8399a01363622f29c839f957823673194e941`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (formatting; the display frame's flush, early frames, the last line, a clock set back).
- **Disposition:** fixed in round 2's commit (see its message).

---

1. **Blocking — [preferences.rs:283](/private/tmp/rv-tr-2/host/linux/src/presenter/preferences.rs:283): required formatting check fails.** Pinned Rust 1.97 `rustfmt --check` exits 1 on the new tuple and queue expressions. **Fix:** format the touched file before landing.

2. **Should-fix — [jni.rs:845](/private/tmp/rv-tr-2/host/linux/src/canvas/jni.rs:845): `displayFrame` flushes before the corresponding hand-off.** This method begins a display tick; [Owner.kt:412](/private/tmp/rv-tr-2/host/android/canvas/src/main/java/dev/exact/canvas/Owner.kt:412) calls it before `produce()`/`Native.frame()`. If a scrub is pending while older agent streams remain queued, it prints `turned` before even those streams are consumed, defeating HEAD’s stale-stream fix. **Fix:** remove this unconditional flush and acknowledge the matching host turn or stream hand-off.

3. **Should-fix — [preferences.rs:284](/private/tmp/rv-tr-2/host/linux/src/presenter/preferences.rs:284): early Android scrubs can use the desktop fallback.** `FRAMES` becomes true only after a callback. With `owner=0` and asynchronous boot, consuming `PRIMED` leaves it false; a scrub subsequently applied by `Native.poll` can print `turned == applied` before Android takes its frame. **Fix:** register frame-reporting capability during host initialization instead of inferring it from a previous callback.

4. **Should-fix — [preferences.rs:290](/private/tmp/rv-tr-2/host/linux/src/presenter/preferences.rs:290): the timeout cannot flush the final pending record.** Expiry runs only when another changed scrub arrives. Android continues polling while the activity is stopped, but stops requesting frames. A final scrub applied then remains pending indefinitely and disappears on process exit; the promised 34 ms fallback never runs. **Fix:** arrange trace-only expiry independently of subsequent scrubs, including while presentation is suspended.

5. **Should-fix — [preferences.rs:292](/private/tmp/rv-tr-2/host/linux/src/presenter/preferences.rs:292): wall-clock rollback defeats queue expiry.** If the device clock moves backward while frames are suspended, the oldest record has a future `applied` time. `take_while` then removes nothing, including newer records behind it, and the uncapped queue grows throughout the setback. **Fix:** use `Instant` for queue age, retain Unix time for output, and impose a capacity bound.

Round 1’s stale-revision filtering, `applied` timestamp placement, poison recovery and non-panicking writes look correct. The disabled steady path adds cached flag checks without trace parsing, queue locking or allocation. Apply and JNI calls share the host thread; `$crate` resolves through public exports. No additional panic path found.

Numeric records match Apple’s field order and Unix-ms units; `turned none` requires nullable handling in the shared bench. Full Android compilation remains unverified under the read-only sandbox.
