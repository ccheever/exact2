# Code review: relay deploy (R4 prep), re-check 3, round 6 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ca2c000ab1`.
- **Method:** one brief (sha256 `c5571ee30ad19de056819803f449351b5e256f8c9dc20bd62241e441c0113312`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. Ubuntu 22.04 (OpenSSH 8.9p1) is inside this script’s support gate, and `sshd -ddd -t` there does not emit `checking syntax for 'Match `. On a syntax check (`connection_info == NULL`), `match_cfg_line` logs `checking syntax for '%s'` with only the criteria, so `Match all` is `checking syntax for 'all'` and `Match User bob` is `checking syntax for 'User bob'`. Other lines still start with `debug`, so the `^debug` guard passes and the Match test does not fire. `sshd -T -C` only samples root and `RELAY_SSH_USER`, so a Match for any other user, group, host, or address can set `PasswordAuthentication yes` for that connection. Jammy stays on 8.9p1. The wording this grep requires, `checking syntax for 'Match %s'`, is what 9.6’s `sshd` contains and what 9.2 logs.

On 9.2 and 9.6 a Match sshd will apply does not get past the check by being parsed only at connection time, or by living in a file read only for some connections. `sshd -t` sends every Match line it reads through that log, and an Include is skipped only inside a Match whose own line was already logged. `Match all` is active during `sshd -t`, so files it includes are read too.

**Non-blocking**

1. With `set -o pipefail`, `grep -q` exits at the first hit and SIGPIPEs `printf` once the debug text is larger than the pipe buffer (~64KB). The `^debug` guard then fails and provision dies with “no debug output”, including on a config with no Match. That stops the run. It also misfires for a long `Match all` body, because `all` is active during `sshd -t` and those directives are logged. The error says nothing changed past the drop-in; if the drop-in was new, sshd was already reloaded.
