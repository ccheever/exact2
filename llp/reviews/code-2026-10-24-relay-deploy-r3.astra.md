# Code review: relay deploy (R4 prep), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c5ad89725e`.
- **Method:** one brief (sha256 `ce37cc9d5c981a8199da6a8f72d651e524456692bda5f3576efff5663a69ebc8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [relay/deploy/remote.sh:37](relay/deploy/remote.sh:37) — **Root follows service-controlled paths.** The accounts bootstrap at line 175 calls `put()` inside the relay-owned state directory. A compromised relay can remove `accounts.json`, then race the next provisioning run by replacing the published file with a symlink before `chmod`/`chown`. Root can thereby transfer an SSH host private key—or another privileged file—to the relay user. Create the accounts file as the service user; avoid root permission changes through paths it controls.

2. [relay/deploy/remote.sh:130](relay/deploy/remote.sh:130) — **The firewall still has a lockout window.** With UFW already active, an incoming-allow default, and no explicit SSH rule, changing the default to deny closes new SSH connections before line 132 adds their allowance. Failure or interruption between those operations leaves the operator unable to reconnect. Install and verify the SSH allowance before changing the default policy.

3. [relay/deploy/remote.sh:120](relay/deploy/remote.sh:120) — **The keys-only check ignores conditional SSH configuration.** `sshd -T` without `-C` does not evaluate connection-specific `Match` blocks. A matching block containing `PasswordAuthentication yes` can therefore remain effective while provisioning passes. Audit/refuse conflicting authentication overrides and validate relevant connection contexts before reloading, including the operator’s context.

4. [relay/deploy/remote.sh:62](relay/deploy/remote.sh:62) — **Firewall inspection errors become successful pruning.** A failed `ufw status numbered` makes the `while` condition false; `set -e` does not abort there, and the function returns success. Extra inbound allowances can remain while provisioning continues. Capture and check the command’s status separately from parsing, and verify the resulting rules before declaring success.

5. [scripts/tamber-relay-gate.mjs:139](scripts/tamber-relay-gate.mjs:139) — **Certificate interception can still yield 7/7.** A mismatch destroys the socket, but `call()` resolves with an empty response on close. `intercepted` is checked only during the first verdict. Intercepting the design request at line 183 therefore records a mismatch, sends no design, and still allows every verdict to pass because that response is ignored. Make mismatches reject the request and fail the run, or enforce an unconditional final interception check.

6. [scripts/tamber-relay-gate.mjs:176](scripts/tamber-relay-gate.mjs:176) — **Line 3 does not establish that its own connection reached the relay.** `forwarded` counts every connection, including connector replenishment. Another successful connection can advance it while the plaintext probe’s upstream connection fails, making the empty close pass. Track upstream connection success for that specific probe.

7. [relay/deploy/deploy.sh:118](relay/deploy/deploy.sh:118) — **The external check still accepts an unknown-SNI timeout.** `probe()` discards the alarm exit status with `|| true`; an unknown-name handshake held open for ten seconds produces no certificate and is reported as closed. Preserve the timeout status and fail the check, as the remote health probe now does.

8. [relay/deploy/remote.sh:322](relay/deploy/remote.sh:322) — **Existing releases can enter first-release cleanup.** When `was == id` but the service is inactive, a failed restart/health check reaches this branch and deletes the existing release and `current`, even if `previous` exists. Subsequent `rollback` also exits at line 335 because `current()` fails. Reserve first-release cleanup for an empty `was`; handle restarting the current release separately and preserve recovery state.

**Non-blocking**

1. [relay/deploy/deploy.sh:57](relay/deploy/deploy.sh:57) — If SSH returns nonzero after producing stdout, `set -e` exits before directory validation, but the EXIT trap uses the already-assigned `UPLOAD` as an `rm -rf` target. Capture status separately and validate before assigning the cleanup target.

2. [relay/deploy/remote.sh:301](relay/deploy/remote.sh:301) — The unprivileged smoke execution has no deadline or resource limits. A broken `--help` can hang deployment indefinitely. Set `RuntimeMaxSec`, `MemoryMax`, and `TasksMax` on the transient unit.

3. [relay/deploy/remote.sh:41](relay/deploy/remote.sh:41) — Mutating commands are not serialized. Concurrent operations share `.new` links and recovery state; certificate pruning can remove another operation’s candidate or rollback directory. Use one host-side lock across provisioning, releases, rollback, and certificate installation.

Both shell scripts passed `bash -n`. Bun tests encountered sandbox `EPERM` restrictions on temporary directories and loopback listeners, preventing runtime validation. No files were changed or remote hosts contacted.
