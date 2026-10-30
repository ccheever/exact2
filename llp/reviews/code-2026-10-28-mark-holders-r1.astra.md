# Code review: kernel: a withdrawn mark report and the kind's shared size (post-hoc follow-up to 9960b11949), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c8e8bc0b32`.
- **Method:** one brief (sha256 `239b8445387e1c89fd61a79320995903e52687c47b93fe7877e964849561a3bc`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

**SHOULD — [kernel/src/kernel/intrinsic.rs:111](kernel/src/kernel/intrinsic.rs:111): The holder’s current key does not establish its report’s key.** Consider:

1. Checkbox A reports 22×22; radio B independently reports 22×22.
2. B becomes a checkbox without reporting again.
3. A withdraws. B now falsely holds the checkbox cache.
4. B becomes a radio again and withdraws. Only the radio cache is cleared.

Unreported checkboxes retain 22×22 with no remaining report; subsequent `None` calls return at line 26 without clearing it. Before this commit, A’s withdrawal cleared that cache. Associate reports with their original key, or invalidate their holder eligibility when the key changes, and cover this sequence.

The ordinary Linux withdrawal pass does clear the cache on the last matching withdrawal, including coincidental holders. Clearing many existing equal reports can take O(n²), but subsequent passes skip the scan through the `None == None` guard. I found no change affecting Apple’s normal positive control reports.

Rust tests were not run: the read-only checkout has no build artifacts.

VERDICT: findings
