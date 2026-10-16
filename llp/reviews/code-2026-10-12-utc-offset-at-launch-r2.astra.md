# Code review: the date at launch (time.utcOffset read 0 at boot) round 2, 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `92e73dc68c9375e15f846819a63135603f885963390cf036a60778f72247b417`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The re-boot priming's dropped commit fixed by a separate launch-date fact (exact_set_launch_time) that never touches the running host; the kept-admission test added (kept_time_tests.rs).

---

- **MAJOR — [host/apple/Sources/ExactKit/SessionTime.swift:51](/Users/admin/x2p2/apps/exact2-d4/host/apple/Sources/ExactKit/SessionTime.swift:51): Reboot priming discards a potentially real commit.** A system-clock correction of at least one second, or an offset change, makes `set_time` update the live runner and emit presentation operations. If the following boot fails, the previous host survives but its presenter never receives those operations. Reporting the same date again commits nothing, leaving the two inconsistent. Apply the returned batch when booted, or prime the next boot without mutating the live runner. Ignoring `"not booted"` before the first boot is safe.

- **MINOR — [contract/cli/tests/it/time.rs:56](/Users/admin/x2p2/apps/exact2-d4/contract/cli/tests/it/time.rs:56): The new test does not exercise kept-answer admission.** It renders `exactTime` using `NoData`; it has no kept snapshot or dependent resource. Add a case with an unready source and an answer kept for `-420`, asserting that the first frame admits it and a mismatched offset refuses it.

The other round-1 fixes check out: `abi.rs` is 1,497 lines, invalid dates preserve the bridge’s last valid date, and the Drop guard restores the previous thread-local value on normal return and unwind, including nested scopes. Both Apple bridge boot paths install time before settlement. The agent still uses `launchPlace.epoch` and the drive zone at `epoch + now()`.

Source tracing confirms that `exactTime` is answered before dependent kept identities are checked. Boots outside `launching` retain zero defaults. Remaining follow-ups include Linux and wasm-web’s post-boot time reporting, direct Apple/content-region and render/terminal entry points, and Apple’s still-default locale/zone/seed before `setPlace`. The web JS target already supplies time independently.

Validation: 26 focused contract tests and 38 kept-answer tests passed using cached binaries built after HEAD; caps and diff checks passed. No rebuild or native app launch was performed.

**NOT READY**
