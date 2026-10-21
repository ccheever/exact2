# Code review: the date at launch (time.utcOffset read 0 at boot) round 3, 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `dfc6f19d2dfa4d3b946e62b6157d89658ace10a53496fc399eb114ffff8147c4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** The NIT (the launch_time field's comment) taken in the landing commit.

---

No BLOCKER, MAJOR, or MINOR findings.

- **NIT — [host/apple/src/abi.rs:110](/Users/admin/x2p2/apps/exact2-d4/host/apple/src/abi.rs:110):** The `launch_time` comment still says `set_time` supplies the retained date. It should name `set_launch_time`; the distinction is central to round 2’s fix.

The fixes check out:

- Runner construction installs the date before settlement and kept-answer matching. Both Apple bridge boot paths use it.
- `primeTime()` refreshes the launch fact before boots and candidate preparation without mutating the live runner or dropping a batch. Priming no longer calls preboot `set_time`.
- `set_time` cannot overwrite the launch fact, including when recommit fails. Invalid launch dates preserve the previous valid fact.
- The Drop guard restores the enclosing thread-local value, including nested calls and unwinding. Boots outside `launching` retain zero defaults.
- The agent calculation preserves `launchPlace.epoch` and evaluates the drive zone’s offset at `epoch + now()`. Both changed files remain below their line caps.

Existing limitations remain suitable for follow-up: [Linux](/Users/admin/x2p2/apps/exact2-d4/host/linux/src/app.rs:341) and the [Rust/Wasm web host](/Users/admin/x2p2/apps/exact2-d4/host/web/glue.js:1359) still supply time after boot. Render, direct Runner/Apple Host entry points, and content-region boots remain zero unless explicitly wrapped. The web JS target already supplies time independently. Apple’s separate place facts also retain their defaults (`seed=0`, `en-US`/`UTC`) until `tellTime()`.

Validation: existing test binaries passed the kept-offset regression, bridge launch-fact regression, and 26 Contract tests, including first-frame time and unwind cleanup. Swift wiring and nested restoration were reviewed statically; no rebuild or UI launch was performed. No files changed.

**READY**
