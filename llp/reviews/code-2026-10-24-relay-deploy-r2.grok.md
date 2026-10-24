# Code review: relay deploy (R4 prep), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b64f9b280d`.
- **Method:** one brief (sha256 `f1d1a7f7ea849014d0ba1a24e2eb464aa62033eb4ac96995875bd92be2958127`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `relay/deploy/remote.sh:185` — `systemctl enable` runs only inside `provision`, and only when `/opt/tamber-relay/current` already exists. `release` (line 251) starts the unit with `restart` and never enables it. The working order is provision, then release, so the relay is up for this boot and disabled. A reboot leaves it stopped. The same provision turns on unattended upgrades (lines 53–56), which is when a reboot shows up.

**Non-blocking**

1. `relay/deploy/deploy.sh:72` — `check` still runs `openssl s_client` with no `timeout`. A blackholed 443 hangs the operator's shell. The remote health probes are bounded; this path is not.
2. `relay/deploy/remote.sh:201` — The key check compares the two `openssl | sha256sum` strings and ignores the pipeline status. If both commands fail, each side is the hash of empty input, the test passes, and the pair is installed. With a release already current, `health` puts the old pair back. With no release yet, the placeholder mark is removed and `install-cert` exits 0.
3. `relay/deploy/remote.sh:207` — The new pair is two renames. If the cert `mv` fails after the key `mv`, `set -e` leaves the function before the restore. The on-disk key is the new one and the cert is the old one; the `.old` copies are still there.
4. `relay/deploy/remote.sh:73` — The keys-only drop-in is written and sshd is reloaded before the port-22 check. A refusal reports that nothing changed, but password login is already off. The check reads `sshd -T`'s `port` line, so a `ListenAddress` or socket unit on another port still passes when the config file says 22, and the firewall then allows only 22.
5. `relay/deploy/remote.sh:263` — A failed first release has nothing to switch back to. `current` stays on that build and `Restart=on-failure` crash-loops it until something stops the unit.
6. `relay/deploy/remote.sh:128` — The unit listens on `0.0.0.0:443` while ufw opens 443 on IPv6 as well. Health only connects to `127.0.0.1`, so an AAAA record fails for clients while health stays green.
