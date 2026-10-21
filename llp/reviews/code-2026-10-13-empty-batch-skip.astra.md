# Code review: applyUnlessEmpty lands published slices, then skips an empty batch, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `416d9e451ad6fcb1f10bfa8bbcedd8a209f30c6db2ee9ec6e5e57c25de53529f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The MAJOR (both: the landing ran the appearance reports before the earlier batch) and Astra's NIT (the QUEUE cause) taken in round 2's code.

---

- **MAJOR — [Session.swift:980](/Users/admin/x2p2/apps/exact2-comp/host/apple/Sources/ExactKit/Session.swift:980): Appearance reports can overtake the waiting batch.** Without `holdingReports`, landing a slice lets `apply` flush `colorsOwed` and `pendingViewDark`. Those reports produce batches *after* the waiting scroll/timer batch. If a report starts a paint transition, it sets `frames.motion = true`; the waiting empty batch’s `motion: false` then fails `changesNothing`, enters `apply`, and stops the transition’s frame updates. Preserve report deferral until the waiting batch’s apply or skip bookkeeping finishes, then drain the reports. Merely setting `holdingReports` would strand them on the skip path. The new test needs this case.

- **NIT — [QUEUE.md:870](/Users/admin/x2p2/apps/exact2-comp/QUEUE.md:870): The display diagnosis is unsupported.** The baseline confirms the device, OS, and all eight failures, but its result bundle also records a missing temporary JPEG error in `PickerColorTests`. Record the cause as unconfirmed instead of “likely the simulator’s display.”

No additional findings in the re-entrancy guards, destruction handling, agent clock, drain callers, or DEBUG seam. The existing Rust test confirming that color reports start motion passed using its compiled binary; Swift tests were not rerun. No files changed.

NOT READY
