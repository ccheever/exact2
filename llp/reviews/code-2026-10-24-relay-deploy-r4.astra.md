# Code review: relay deploy (R4 prep), re-check, round 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3023abe3c2`.
- **Method:** one brief (sha256 `7cc44c18eb1436f95729a5768679a44adb5f6fe4b9de72fad98a4303a3144bee`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [relay/deploy/remote.sh:134](relay/deploy/remote.sh:134) — **Fix 3 remains incomplete.** Valid directives such as `PasswordAuthentication "yes"`, `PasswordAuthentication=yes`, and `ChallengeResponseAuthentication yes` bypass the Match scanner. In a block matching another source address, they also escape the current-connection `-C` checks, allowing provision to succeed with password authentication enabled elsewhere. Local parsing confirmed these forms are accepted by sshd but missed by the scanner. Handle sshd’s syntax, aliases, and recursive Include context, or conservatively refuse configurations that cannot be validated.

2. [scripts/tamber-relay-gate.mjs:88](scripts/tamber-relay-gate.mjs:88) — **Fix 6 retains stale connection evidence.** `reachedFrom` permanently remembers successful client ports. If the plain probe reuses one and its upstream connection fails, the empty close plus stale membership makes line 3 pass. An in-memory replay reproduced this. Replace the entry on every accepted connection with a fresh record, and require that record’s upstream connection to have succeeded.

**Non-blocking**

none. No additional blockers found in the other fixes. Review was read-only and offline; deployment checks were not rerun.
