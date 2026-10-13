# Code review: launch facts round 3, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `5bd3acc58996f6fb14d4f65db5ded4d7da9707c7f859bb5876597a816f4ed243`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

READY**

No issues. The round-2 claim holds: only a renderer-clock checkpoint drops the launch date via `without_launch_time`; a driven, unreadable, or elsewhere-rendered boot stays under the page date, the place is preserved, and the outer launch is restored. The Linux QUEUE note matches the code.No user-visible defects in `50cb321` (`git show HEAD`). Nothing a user or an app would hit.

The round-2 wasm fix holds. `Bridge::boot` passes `launch_facts` whole into `launching_with` (`host/web/src/abi.rs:289`). `Host::boot_checkpoint_linked` then splits on the checkpoint:

- An unreadable checkpoint and one rendered at another path return through `Host::boot_linked` while that outer launch is still in force (`host/web/src/page.rs:482`, `host/web/src/page.rs:492`), so the page date stays and the clock starts at 0.
- A driven checkpoint forces `now_ms` to 0 and calls `boot()` under that same launch (`host/web/src/page.rs:477`, `host/web/src/page.rs:523`).
- Only a checkpoint that actually boots on its renderer's clock runs under `without_launch_time` (`host/web/src/page.rs:526`). That nests `launching_with` with the date cleared and the current place kept (`runner/src/time.rs:96`). The boot copies that fact; `Drop` restores the outer launch, and nothing after the boot reads the thread-local again.

The Linux offset is unchanged and the QUEUE sentence matches the code: the date is sampled in `boot_presenter` before the runner boot (`host/linux/src/app.rs:321`), while the display loop's clock zero is the earlier `started`, taken before `Display::open` (`host/linux/src/app.rs:526`, `host/linux/src/display.rs:327`).

READY
