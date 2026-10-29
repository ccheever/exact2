# Code review: relay deploy (R4 prep), re-check, round 4 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3023abe3c2`.
- **Method:** one brief (sha256 `7cc44c18eb1436f95729a5768679a44adb5f6fe4b9de72fad98a4303a3144bee`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `relay/deploy/remote.sh:134` (effective check at `:140`). The scanner does not refuse every Match that turns password login back on. `FNR == 1 {m = 0}` clears Match at each file, but sshd keeps a Match open across `Include`d drop-ins, so a `Match User ubuntu` in one file and `PasswordAuthentication yes` in a later one is invisible here. `PasswordAuthentication=yes` (OpenSSH treats `=` as a delimiter) and `ChallengeResponseAuthentication yes` never match `$1`/`$2` either. `sshd -T -C` only evaluates `root` and `RELAY_SSH_USER`, and it sets `host` to the client IP, so a `Match Host` that applies when `UseDNS yes` is not the policy of this connection. A same-file `PasswordAuthentication yes` is refused; a split drop-in or `keyword=value` for any other user is not. Scan one stream in sshd's include order, split keywords on whitespace or `=`, treat `ChallengeResponseAuthentication` as the keyboard-interactive alias, and run `sshd -T -C` for every login-capable user. When `usedns` is yes, pass that client's canonical hostname as `host`, not only the address.

**Non-blocking**

none
