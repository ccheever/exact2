# Code review: relay deploy (R4 prep), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b64f9b280d`.
- **Method:** one brief (sha256 `f1d1a7f7ea849014d0ba1a24e2eb464aa62033eb4ac96995875bd92be2958127`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [relay/deploy/remote.sh:186](relay/deploy/remote.sh:186) — **The first release is never enabled at boot.** Fresh provisioning skips `systemctl enable`, and `release` only restarts the service. The documented provision → release sequence succeeds, but the relay stays down after reboot. Enable it after the first successful release.

2. [relay/deploy/remote.sh:207](relay/deploy/remote.sh:207) — **Certificate replacement is still two separate renames.** If the key rename succeeds and the certificate rename fails, `set -e` exits before restoration; the EXIT trap only deletes uploads. An interruption between renames has the same result: a mismatched installed pair that prevents the next start. Use an atomic switch between directories containing complete pairs.

3. [relay/deploy/remote.sh:83](relay/deploy/remote.sh:83) — **The SSH guard checks configuration, not the live listener.** An sshd launched with `-p 2222`, or socket-activated on that port, can still yield `port 22` from this separate `sshd -T` invocation. Provisioning then blocks the actual SSH port. Check the live listener and server-side connection port before replacing firewall rules.

4. [relay/deploy/remote.sh:232](relay/deploy/remote.sh:232) — **A failed symlink creation is silently bypassed.** Because `ln` precedes `&&`, its failure does not trigger `set -e`; `switch` proceeds to restart the unchanged release. Deployment can report success without switching. During recovery, the caller can subsequently delete the failed release while `current` still points to it. Propagate switch failures and verify restoration before deletion.

5. [relay/deploy/remote.sh:304](relay/deploy/remote.sh:304) — **An unknown-SNI timeout passes health.** `|| true` discards timeout status, and any output without a certificate is treated as a closed connection. A release that holds unknown connections indefinitely therefore passes and avoids rollback. Preserve timeout detection separately from OpenSSL’s handshake-error status.

**Non-blocking**

1. [relay/deploy/deploy.sh:51](relay/deploy/deploy.sh:51) — Upload cleanup only exists inside the remote handler. An interrupted upload, failed SSH connection, or sudo refusal before that handler leaves the private upload directory and potentially the key behind. Permissions prevent the earlier shared-`/tmp` exposure, but cleanup is not unconditional.

2. [relay/deploy/deploy.sh:72](relay/deploy/deploy.sh:72) — Both external `check` TLS probes remain unbounded. A peer accepting TCP without completing TLS can hang this command indefinitely; the timeout fix only covers remote `health`.

3. [scripts/tamber-relay-gate.mjs:105](scripts/tamber-relay-gate.mjs:105) — The poll counter measures handler entry through completion, including body parsing, rather than registration of a held long poll. Observing actual waiter registration would make the claimed revocation evidence precise.

`bash -n` passed. The Bun test run encountered sandbox `EPERM` failures for temporary files and loopback listeners, so runtime verification remains unavailable here.
