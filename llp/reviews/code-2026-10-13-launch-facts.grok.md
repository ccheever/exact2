# Code review: the date and place at launch on Apple, the wasm host and Linux, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `561be9a344063a14163ad947620ef22e5bb8be32be1efd0a32654bc560b3b5be`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's two MAJORs (the locale after the initializers; a renderer-clock checkpoint's date) and Grok's (Linux's two date samples) taken in round 2's code.

---

**MAJOR** — The Linux non-agent launch epoch is sampled before the runner exists, and the post-boot tell uses a later clock — `host/linux/src/app.rs:320`

`boot_presenter_with_painter` stores an absolute `SystemTime::now()` at line 320, before `module.replacement` (line 395), font load, painter open, and `Host::boot_decoded`. That value is what the first settle reads, through `launching_with` on both the selected boot (line 401) and the baked fallback (line 443). The fallback keeps the sample taken before the failed attempt. `delivered` then calls `set_time` with a new `SystemTime::now() - host.now()` (lines 348–356). `now_ms` is still 0, so this is the wall time at the end of boot. `images.prepare_metadata` may block for a full second after that first settle and before `delivered` (`host/linux/src/presenter.rs:408`). `set_time` treats a gap under 1000 ms with the same offset as the same fact and commits nothing (`runner/src/runner/time.rs:61`), so a faster boot freezes the early epoch for the process: the first frame’s date is the pre-runner sample, and the correction the comment at line 318 says will commit nothing is discarded. At 1000 ms or more, `set_time` recommits every `exactTime` resource, so a kept answer whose identity includes the epoch is asked twice. The comment that the later tell commits nothing does not hold on that path. The agent path is unaffected: one `agent_time` tuple is used for both the launch fact and `delivered`.

## Checked, no further findings

The launch thread-local restores the outer `Launch` on return and on unwind. `launch_time` and `launch_place` copy out of the borrow before the boot runs. A nested `launching` passes `place: None` and would hide an outer place only for that inner boot; the only callers are tests, and no production boot nests it inside `launching_with`.

The locale-slot write at `runner/src/runner.rs:899` runs after `init_slots` and before settle, derives, and the language op. It uses `resolve_locale` and `Value::str`, the same write `set_place` does. A plan with no locale slot, or a locale that does not resolve, skips the write; `exactTime` still comes from `runner.place`. A carried locale is restored by `init_slots` and then overwritten, which is the end state of a later `set_place`. Slot initializers have already run, so a `t(...)` inside one still sees the base or carried locale. `set_place` does not re-run initializers either.

Every production boot named here calls `launching_with` on the thread that constructs the runner. Linux wraps `Presenter::boot_selected` at lines 401 and 443, and `boot_decoded` builds the runner on that stack. The wasm host does the same around `Host::boot_checkpoint_linked` (`host/web/src/abi.rs:290`) and `Host::boot_linked` (`:338`); `boot_module` ends in `boot_plan`. Apple does it around `Host::boot_stored_after_decode` (`host/apple/src/abi.rs:540` and `:842`).

The web glue copies the place out of the shared input buffer before the boot payload reuses it. `writeIn(reportPlace())` runs as an argument to `exact_set_launch` (`host/web/glue.js:1305`), and `set_launch_place` parses that buffer into an owned `Place` before returning (`host/web/src/abi.rs:764`). The later `exact_in` for the module, plan, or checkpoint payload is safe. Apple’s `write` and `exact_set_launch_place` run in one synchronous owner-thread call.

The agent clock and the drive place match on the hosts this commit changes. Linux reuses the captured agent epoch, the offset at that epoch, and the place (seed included). The web reporter uses the drive epoch and the offset at `epoch + elapsed`, with the agent clock starting at 0, and `followOffset(now())` after boot uses that same reporter. Apple’s `dateNow` under the agent returns the drive epoch, and the post-boot tell uses the same formula. The web non-agent epoch is `Date.now() - elapsed` on both sides, so the await in `bootNow` does not skew it.

`host/web/glue.js` and `host/apple/src/abi.rs` are 1499 lines. `host/web/src/abi.rs` is 1480. The project cap is 1500. The JS target still answers `exactTime` from the page and is unchanged. The render host, the content-region host, a direct `Host::boot_stored`, and the Linux candidate reload remain outside this boot, as QUEUE says.

The new tests cover a place read on the first frame, an unchanged place committing nothing, an invalid place leaving the defaults, the last valid Apple launch place, and the wasm bridge’s first batch carrying the language op and the offset. Nothing asserts that the Linux post-boot `set_time` commits nothing.

NOT READY
