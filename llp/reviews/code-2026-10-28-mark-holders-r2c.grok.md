# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), round 2 re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `344cca6c5a`.
- **Method:** one brief (sha256 `6702413e8a67af33c846b8ab68157af211f9e6b1aa2a870469e889e33cafbcbe`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll re-check the two round-2 fixes in `344cca6c5a` against the full change from `origin/main`, read-only, and look for anything the early-return path now does to mark reports.This is a focused read-only check of those two fixes, not a full branch review. I'll read the follow-up commit and the surrounding mark-report path before judging either fix.The follow-up puts `note_mark_report` on the unchanged-size return. Next I’ll read that path and how Apple and Linux call it, to see whether every repeat now writes `mark_reports`.**SHOULD** — Retargeting an unchanged report leaves the previous kind's shared size with nobody to withdraw it. `kernel/src/kernel/intrinsic.rs:28`

`set_intrinsic_size` now calls `note_mark_report` and returns when the size is unchanged. That rewrites `mark_reports` to the mark's current kind and font, then skips `mark_withdrawn`. `mark_sizes` for the previous key stays, and no remaining record is under that key, so a later `None` withdraws only the new key. A second radio with no size of its own keeps the 22 pt a radio reported and then reported again as a checkbox — the case `a_mark_holds_a_shared_size_by_the_kind_it_reported_as` adds. Layout reads that shared size at `kernel/src/layout.rs:1159`. The same return also skips `mark_reported`, so the re-report does not become the new kind's shared size unless some other mark of that kind already reported it. Withdraw under the stored key when it differs, then note the new key.

The repeated-report path is sound when the key is unchanged. An image's repeat (Apple) resolves `mark_key` as a non-control, misses in `mark_reports`, and returns before `intrinsic_changed`, so layout stays clean. Linux's repeat of `None` from `size_controls` (`host/linux/src/presenter/control.rs:432`, every `finish_commit` for a neutral or Material mark) removes an absent record and does not layout; a `None` that actually withdraws still sees `Some` and takes the main path, which calls `mark_withdrawn` before dropping the record.

The line-cap fix holds. `kernel/tests/it/browser_controls.rs` is 1286 lines (1517 at `2eb32ba82f`). The three mark-size tests live in `kernel/tests/it/mark_sizes.rs`, registered at `kernel/tests/it/main.rs:42`, and the same-size case is the last assertions of `a_mark_holds_a_shared_size_by_the_kind_it_reported_as`.

VERDICT: findings
