# Code review: setRootFontSize("125%"), round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `b5bb95021d3c6f1b12ad1bc544f455b143341d781ad04fdf86980eaefad32c3a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (one MINOR); Grok: ship.
- **Disposition:** the JS target computes the ratio first, as the runner does, with a test at hosts 18 and 24

---

No findings.

The three Round 1 fixes match the runner, and the new tests cover them.

`css_percent` now divides by 100 before the finite-and-positive check, so `"5e-324%"` is `None` instead of `Some(0.0)`. Any finite `f64` divided by 100 either stays a positive finite ratio or underflows to 0 and is rejected. The compiler uses that result, so the literal is rejected there too. A zero ratio could never pass the runner’s `fits(host * ratio)` check.

`appRootFontSize` refuses a percentage unless `Math.fround(host * n / 100)` is a finite `f32` above 0, which is the same gate as `fits` in `runner/src/runner/root_font.rs`. The host is read with the app rule disabled, so a standing px or percentage rule is not treated as the host. At every valid host, `"1e300%"` overflows and `"1e-100%"` underflows. The `f32` boundary for host 16 agrees on both sides (`n` about `2.126764729873335e39`). Px values and `"medium"` are unchanged. The check is stricter than the old ratio test, so it does not accept anything that used to be refused.

The runner test is a real overflow of the share: with `125%` active, `3e38` fits as an `f32` host and `3e38 * 1.25` does not. The refusal leaves the root at 25 and the host at 20. `"1e-100%"` and `"125"` then leave that percentage in place, so the next host of 16 resolves to 20.

The waived rollback is already in the snapshot. `commit.rs:112` stores the whole root-font value (the percentage ratio and the host) plus the kernel size, and `commit.rs:182` restores both. That snapshot is taken at the start of the action, before `setRootFontSize` runs. A failed host update while a percentage is active is put back by the restore in `root_font.rs` after `commit_again`; that path does not change the percentage.

`css_percent`, the two web root-font tests, and `the_apps_percentage_follows_the_hosts_size` all passed.

VERDICT: ship
