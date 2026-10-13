# Code review: launch facts round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `70cde61f48bb69c54fea1aef46938d000c63a72c91a1280b1a8de9978aa4d4cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR (every checkpoint dropped the date) taken in round 3's code; Grok's (the Linux loops' clock zero) pre-existing in kind, recorded in QUEUE.

---

- **MAJOR — [host/web/src/abi.rs:293](/Users/admin/x2p2/apps/exact2-comp/host/web/src/abi.rs:293): Preserve launch time for checkpoints that boot at zero.** This condition drops the date for every checkpoint. However, [page.rs:477](/Users/admin/x2p2/apps/exact2-comp/host/web/src/page.rs:477) resets agent checkpoints to zero, and invalid or differently located checkpoints fall back to an ordinary boot. Those paths still first settle with `epochAtZero=0` and `utcOffset=0`, rejecting correctly keyed kept answers before the later `set_time`. Restrict date suppression to boots that actually retain the renderer’s clock.

New launch tests, targeted page/kept-answer tests, glue buffer-order checks, and caps passed. Filesystem-backed fixtures were blocked by the read-only sandbox. No files changed.

NOT READY
