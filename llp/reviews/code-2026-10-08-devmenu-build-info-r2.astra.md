# Code review: build info in the development menu, round 2 (d5669cb9f), 2026-10-08 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree.
- **Method:** one brief (sha256 `e105755fef7655aa9f832a51c72425142eb7345505e897a82b9e7203cfd6e077`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: `--untracked-files=normal`, tested with `status.showUntrackedFiles=no`. 2 taken: the formatter takes the current zone at each opening (not tested across two zones). 3 taken: the Swift test is named for the iOS lane (`BuildInfoIOSTests`), and the Bun test runs from the async lane's `tests/it/development.rs`.

---

LAND WITH CHANGES

1. **MATERIAL — Untracked-file fix remains configuration-dependent.** [buildinfo.mjs:16](host/apple/buildinfo.mjs:16) uses `status --porcelain`, which still obeys `status.showUntrackedFiles=no`. New source files can therefore stamp as clean. Reproduced read-only. Pass `--untracked-files=normal` explicitly and test with that configuration disabled. [Git documentation](https://git-scm.com/docs/git-status).

2. **MINOR — Local time becomes stale after changing time zones.** [BuildInfo.swift:37](host/apple/Sources/ExactKit/BuildInfo.swift:37) caches a formatter with `.current` once. Subsequent menu openings and Copy retain the old zone. Refresh its time zone before formatting; test two zones with independently specified expected strings. A Foundation probe reproduced this behavior.

3. **MINOR — New tests miss the documented async lane.** [build.mjs:1477](host/apple/build.mjs:1477) selects only `*IOSTests.swift`, excluding `BuildInfoTests`; [async.mjs:75](scripts/async.mjs:75) discovers Bun tests only under `host/web`. Rename the Swift file/class accordingly and move the Bun assertions into the existing [Apple async test wrapper](host/apple/tests/it/development.rs:24).

Other round-1 fixes check out: 1,499-line cap, fractional age, bare-macOS sidecar, iOS host stamp, release classification and POSIX locale. Copy includes the displayed fields; tvOS retains its pasteboard guard. Stamping precedes signing and preserves compiler reuse; timestamps make packaged bytes nondeterministic. Apple [permits custom plist keys](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/AboutInformationPropertyListFiles.html#//apple_ref/doc/uid/20001431-102346).

Caps, the read-only Bun test, and no-repository/no-Git probes passed. Native builds, Swift tests and the write-dependent Git fixture were not run.
