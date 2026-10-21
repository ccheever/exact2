# Code review: stage 10c R3, the relay service, round 3 re-check (403489af9a, 971e3485c5) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `88cb7c90e2`.
- **Method:** one brief (sha256 `d59318570b8ea88d08e6573973dbfdc69977ed0711074488ea13bc0cd639e3b2`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [tamber-relay-server.mjs:400](scripts/tamber-relay-server.mjs:400): **Unauthenticated clients can prevent BYO restart recovery.** After the old connector disconnects, a ClientHello to its public name creates a waiter. `held.waiting.size` then prevents reclamation during grace. The restarted connector receives `REFUSED` and [stops permanently](scripts/tamber-relay.mjs:276), even after that waiter expires. Reclamation should transfer or close waiters when no old connector remains.

- [tamber-relay-server.mjs:387](scripts/tamber-relay-server.mjs:387): **The session cap runs before reclamation.** At `sessionsPerAccount` capacity, the old session in grace still occupies a slot, so its replacement is refused before reaching the takeover code. This breaks routine restarts at the default four-session limit, or with a configured limit of one. Apply the cap after accounting for the session being replaced.

**Should-fix:** None additional.

**Nits**

- [tamber-relay-service.md:41](docs/tamber-relay-service.md:41): Anyone can grind the public, unkeyed 48-bit hash; only the account’s token holder can **claim the resulting name**. Correct that wording. The account-boundary rationale otherwise holds: neither ownership takeover is available without the account’s token.

The other listed fixes look correct in source. Runtime verification was blocked by sandbox `EPERM`: **4 tests passed, 49 failed** on socket/temp-directory setup; the gate also failed during setup. No files changed.
