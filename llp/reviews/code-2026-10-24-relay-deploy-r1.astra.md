# Code review: relay deploy (R4 prep), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `97e3215fbc`.
- **Method:** one brief (sha256 `3c312bf1c50aa56eeceb85753f3b3795c2c6a7fe6ece32489639bc80069e84ce`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [relay/deploy/remote.sh:223](relay/deploy/remote.sh:223) — **Restart failure bypasses rollback.** `switch` changes `current`, then runs an unguarded `systemctl restart`. If that command fails, `set -e` exits before the recovery branch. The new release remains selected. Switching, restarting, and health checking need one recovery path.

2. [relay/deploy/remote.sh:191](relay/deploy/remote.sh:191) — **Certificate replacement can leave the service down.** `-checkend 0` checks expiry, not `notBefore`. A future-dated certificate passes these checks, replaces the working certificate, then fails the relay’s startup validation. There is no restoration of the old pair. The two separate renames also permit a mismatched pair after interruption.

3. [relay/deploy/deploy.sh:44](relay/deploy/deploy.sh:44) — **The uploaded private key has no guaranteed protection.** A source key with mode `0644` can become readable by other users in `/tmp`; the later `install -m 0640` does not protect that upload. Validation failures also leave it behind. Upload into a private temporary directory and clean it on failure.

4. [relay/deploy/deploy.sh:34](relay/deploy/deploy.sh:34) — **Arguments become remote shell syntax.** `$*` joins arguments without escaping them for SSH’s remote shell. A quoted local label containing `;` or command substitution executes remotely before label validation, as root for a root connection. Serialize each argument safely or validate it locally before constructing the command.

5. [relay/deploy/remote.sh:88](relay/deploy/remote.sh:88) — **Provisioning can lock out SSH on a nonstandard server port.** The client accepts `RELAY_SSH_PORT`, but provisioning neither confirms that sshd listens on 22 nor changes its port. If sshd actually uses another port, enabling this firewall prevents reconnection. Verify server-side port 22 before replacing the rules; an external VM forwarding port is a separate case.

6. [relay/deploy/remote.sh:266](relay/deploy/remote.sh:266) — **TLS health probes have no deadline.** A bad release that stays alive and accepts TCP but stalls the TLS handshake can hang deployment indefinitely, preventing automatic rollback. Bound both `s_client` probes.

7. [relay/deploy/remote.sh:268](relay/deploy/remote.sh:268) — **The unknown-name check can falsely pass.** Under `pipefail`, `s_client` returning nonzero after printing a certificate makes the entire condition false—even when `grep` matched. Early `grep -q` termination can also cause SIGPIPE. That becomes “unknown name is closed.” Inspect captured output independently of the producer’s exit status; `deploy.sh:67` has the same problem.

8. [scripts/tamber-relay-gate.mjs:122](scripts/tamber-relay-gate.mjs:122) — **The deployed capture does not establish end-to-end TLS.** The tap faithfully captures forwarded wire bytes, but a relay terminating and re-encrypting TLS would also produce an encrypted capture and could pass using another publicly trusted certificate. Compare the client-observed certificate/SPKI with the local design server’s advertised fingerprint/SPKI before claiming this gate establishes pass-through.

9. [scripts/tamber-relay-gate.mjs:144](scripts/tamber-relay-gate.mjs:144) — **Revocation can pass without cutting a pending poll.** Over a real network, an empty response after 300 ms can mean the request has not arrived. Revoking then produces a normal authentication rejection, satisfying the assertion without exercising cancellation. Wait for server-side registration of the poll before revoking.

**Non-blocking**

1. [relay/deploy/deploy.sh:30](relay/deploy/deploy.sh:30) — An unqualified host is assumed to use root, although SSH may select the local username or a configured `User`. This incorrectly skips sudo. Explicitly select root or determine the effective remote user.

2. [relay/deploy/deploy.sh:71](relay/deploy/deploy.sh:71) — “Only 22 and 443 answer” overstates the check: five other ports are tested, and 22 is not tested. Report the actual coverage.

Both scripts passed syntax checks. The Bun suite encountered sandbox `EPERM` restrictions on temporary directories and loopback listeners, preventing runtime validation.
