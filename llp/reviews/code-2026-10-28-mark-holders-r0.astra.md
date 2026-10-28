# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), post-hoc round (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9960b11949`.
- **Method:** one brief (sha256 `b354d721858b649072d11c97fd14b41c0907482379ad1e874e88bc779cc88055`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **SHOULD — [kernel/src/kernel/intrinsic.rs:105](kernel/src/kernel/intrinsic.rs:105): Equal dimensions do not establish ownership of the shared report.** Let A report 22×22, B report the same, and C remain unreported, all under one key. Withdrawing B deletes the shared entry seeded by A and shrinks C to the default, although A’s report remains valid. Re-reporting A’s unchanged size cannot restore it because line 26 returns early. Preserve shared sizes supported by other reports; a per-node withdrawal should not imply that the host stopped sizing the whole kind.

- **SHOULD — [kernel/src/kernel/intrinsic.rs:101](kernel/src/kernel/intrinsic.rs:101): Withdrawal uses the current key rather than the report’s key.** Style/prop changes retain the node’s intrinsic. If A reported 22×22 at font-size 16, then changes to font-size 20 and withdraws, it can delete an independently populated `(Checkbox, 20)` entry containing 22×22. Unreported font-size-20 peers then resize despite never sharing A’s report. A switch→checkbox change has the same problem when dimensions coincide. Associate withdrawal with the key under which the report was made.

The sizing reader is [layout.rs:1159](kernel/src/layout.rs:1159). Measurement APIs reuse that layout path; arena copies preserve the table. Host Rust, accessibility and exported snapshots have no independent shared-size lookup—they observe resulting geometry.

Apple’s iOS/macOS mark publishers send positive per-node sizes, without withdrawing marks, so I found no direct Apple regression from this fix. Linux’s `size_controls` withdraws reports for kernel-sized kinds; Windows and Android reuse that presenter. `appearance: none` is excluded from sharing and eviction. Peers’ own intrinsics remain intact; explicitly sized, unreported peers can still be unnecessarily dirtied, although authored dimensions win.

Source review only; tests were not run in the read-only checkout.

VERDICT: findings
