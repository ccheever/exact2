# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `2eb32ba82f`.
- **Method:** one brief (sha256 `4ada0ea68afa78bade0f110005d19cc6f3f468e3ded8bfb25c48924a8ff13b4e`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the two follow-up commits against main's shared mark sizes, read-only, and check whether `mark_reports` stays consistent on every path.This is a targeted correctness pass on those two commits, not a full branch review. I'll read the diff and the mark-report paths around it.The diff is small. Next I’ll read every path that writes or reads `mark_reports`, including early returns in `set_intrinsic`.No findings.

`mark_reports` stays aligned with the report that is actually in force. A positive mark report records the key at that moment, including `mark_reported`'s fast path, which runs only after the insert. Withdrawal reads the stored key before the map entry is removed. A `None` report, a non-mark report, `free_slot`, and `reset` all drop the entry. Clone copies it. The early returns in `set_intrinsic_size` (unchanged size, `measures_symbol`, refused size) leave the map alone, which is right: those calls do not replace the report, and a mark is never a measured symbol (`symbol_role` is image-only). A freed slot cannot come back still holding an entry. `alloc` only reuses slots that `free_slot` has already cleared.

An `IdMap` insert on each checkbox's first report is a normal hash-map insert. Apple reports a control when its natural size changes, not on every sync, and a repeat of the same size returns before the insert. Steady-state Linux withdrawals are `None` on an intrinsic that is already `None`, so they return at the same check.

The three tests lock the two behaviors this follow-up is for. Two equal reports, then one withdrawal, leaves the size. A radio that reported the checkboxes' size and then became a checkbox does not hold that size, so the remaining checkbox withdrawal takes it back to 13. The switch-turned-checkbox case in the middle test still has another checkbox reporting, so the later test is the one that distinguishes the stored key.

VERDICT: clean
