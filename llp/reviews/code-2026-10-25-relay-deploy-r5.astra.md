# Code review: relay deploy (R4 prep), re-check 2, round 5 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `41eeeacaa4`.
- **Method:** one brief (sha256 `1385596a31ebcb66059193ed9c98e61c7c42839950ed69b2bbfb636667251b7d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [relay/deploy/remote.sh:142](relay/deploy/remote.sh:142) — **Quoted keywords bypass the Match prohibition.** OpenSSH accepts `"Match" User somebody` followed by `PasswordAuthentication yes`; the regex misses it. If `somebody` differs from root and the operator, both effective-setting checks pass. Decode keywords using SSH-compatible tokenization before rejecting `Match`, or fail closed on unsupported syntax.

2. [relay/deploy/remote.sh:146](relay/deploy/remote.sh:146) — **Quoted Include paths escape recursive scanning.** `Include "/etc/ssh/extra.conf"` retains its quotes, becomes an incorrect path, and is silently skipped. OpenSSH reads the real file, where a password-enabling `Match` for another user can escape both checks. Quoted `"Include"` keywords are also missed. Parse Include keywords and arguments using SSH-compatible quoting before resolving paths and expanding globs.

**Non-blocking**

none. The gate’s fresh-record fix closes stale-port reuse, including delayed callbacks from older connections; the in-memory checks passed.
