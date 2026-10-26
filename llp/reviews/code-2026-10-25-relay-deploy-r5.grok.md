# Code review: relay deploy (R4 prep), re-check 2, round 5 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `41eeeacaa4`.
- **Method:** one brief (sha256 `1385596a31ebcb66059193ed9c98e61c7c42839950ed69b2bbfb636667251b7d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `relay/deploy/remote.sh:142` — A `Match` keyword or `Include` path in double quotes is real `sshd` syntax and is invisible to this walker, so password login can stay on for every connection the provisioning session does not sample. `"Match" Address 10.0.0.0/8` is parsed as `Match Address 10.0.0.0/8` (`sshd` reports `checking syntax for 'Match Address ...'`), while `grep` requires a bare `match` token, so the file is opened and the block is kept. The same is true of an include the `sed | tr ' '` split never opens, which `sshd` does open: `Include "/etc/ssh/extra.conf"`, `"Include" /etc/ssh/extra.conf`, `Include "/etc/ssh/evil name.conf"`, and (current OpenSSH) `Include /etc/ssh/evil\ name.conf`. A plain `Match` in that file turns `PasswordAuthentication` back on for the addresses it names. `sshd -T -C` for root and the operator from this source address still prints `passwordauthentication no`, and provision continues. Tokenize each line the way `sshd` does (double quotes stripped, `=` separating the keyword, a backslash keeping the next character, including space), and refuse a first token of `match` in any case. Resolve `include` arguments only after that, then glob. Or take the file list and the `Match` lines from `sshd -ddd -t` (`including <path>`, `checking syntax for 'Match ...'`), which is the set this `sshd` actually applied.

2. `relay/deploy/remote.sh:140` — The already-seen test uses the path as a glob pattern, so an included file whose name contains `*`, `?`, or `[` is skipped while `sshd` still reads it. After `/etc/ssh/sshd_config` is recorded, a real file named `/etc/ssh/sshd_*` matches `${seen#* $f }` and is dropped (`[ -f ]` is true). `Include /etc/ssh/sshd_*` opens that file, and a `Match` inside it re-enables password login for other connections. Track seen paths with `declare -A` and compare the literal string.

**Non-blocking**

none
