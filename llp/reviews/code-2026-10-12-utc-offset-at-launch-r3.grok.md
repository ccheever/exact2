# Code review: the date at launch (time.utcOffset read 0 at boot) round 3, 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `dfc6f19d2dfa4d3b946e62b6157d89658ace10a53496fc399eb114ffff8147c4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The NIT (the field and module comments) taken in the landing commit.

---

**READY.** One nit. The four commits do what round 2 asked: the first settle on Apple reads the real date, and priming a boot no longer touches the running runner.

### NIT
`host/apple/src/abi.rs:110` — `launch_time` is still documented as the date last told by `set_time`, kept the way preferences are. `set_time` (`host/apple/src/abi/preferences.rs:24`) only tells the running host. Only `set_launch_time` (`preferences.rs:36`) writes the field, and only after `WallTime::validate`. The module comment at `preferences.rs:1` still says the date is both told and kept. The function comments under it are the ones that match the code.

### What holds

The first settle reads the primed date. `Runner` is built with `time: launch_time()` (`runner/src/runner.rs:870`) inside `boot_inner`, before `settle`. `exactTime` is answered from that field (`runner/src/runner/query.rs:28`). Kept admission compares those evaluated arguments with the stored key (`runner/src/runner/settlement.rs:437`). `kept_time_tests.rs` admits a `[-420]` answer under `launching(Some(-420))` and shows the placeholder for `None` and for `+60`. The contract test shows the first frame text `1790000000000/-420`, and the same fact told again commits nothing.

Every Apple ABI boot goes through `launching`: `boot_fresh` (`abi.rs:541`) and `prepare_plan_carried` (`abi.rs:842`), so `boot_selected`, `boot_plan`, and both prepare entries are covered. `primeTime` runs in `boot(size:)`, `boot(plan:)`, and `prepare` (`Session.swift:779`, `806`, `866`), and `prepare` is what file watch, the dev server, and `applyTogether` use. A candidate therefore settles under `dateNow()`, including the agent's frozen `launchPlace.epoch` and that zone's offset. `set_launch_time` never calls the host, so a re-boot's prime leaves the live runner, its dispatched requests, and its unpresented batch alone.

A boot outside `launching` still gets `WallTime::default()` (`0` / `0`). The thread-local is restored by a `Drop` guard (`let _restore`, so it is not dropped early), including on unwind, and a nested `launching` puts the outer date back. The contract test covers the unwind. `set_time` on an unbooted bridge still returns the `not booted` batch; the session no longer calls it before boot. `primeTime` calls `setLaunchTime` and ignores that return.

An invalid date is not stored, so a value `validate` accepts and a later `set_time` recommit then rolls back cannot become the next boot's date. `set_launch_time` and `launching` both use `WallTime::validate`.

### Still zero at the first settle

Left as they are. Linux tells the date after the runner exists (`host/linux/src/app.rs` around the post-boot `set_time`, and `restore_time` on a candidate). The web page calls `exact_set_time` from `followOffset` after `exact_boot` (`host/web/glue.js:1359`). The render host and `Host::boot_region` / `Host::boot_stored` never enter `launching`. Place is still `tellTime` after the boot batch, so the first settle still sees seed `0` and `en-US` / `UTC`. Page facts are told on that same pass. Preferences are already on the first viewport.

READY
