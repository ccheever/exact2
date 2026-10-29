# Code review: relay deploy (R4 prep), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `97e3215fbc`.
- **Method:** one brief (sha256 `3c312bf1c50aa56eeceb85753f3b3795c2c6a7fe6ece32489639bc80069e84ce`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `relay/deploy/remote.sh:203` and `relay/deploy/remote.sh:221`. `switch` moves the `current` symlink and then runs `systemctl restart`. That restart is not in a conditional, so if it returns non-zero, `set -e` ends the script inside `switch` before `if health` at line 224. A binary that exits as soon as it starts (bad config, missing file, `203/EXEC`) makes `systemctl restart` fail on Ubuntu and Debian after the symlink already points at the new release. The automatic switch-back and the restore of the old `previous` value (lines 229–233) never run. The service is down or crash-looping on the new id. `previous` still names the old id, so a later `rollback` can recover, but this path does not do it itself. The same `switch` is what `rollback` uses at line 247, so a restart failure there also skips its health check.

2. `relay/deploy/deploy.sh:34`. The remote command is one string, `"$sudo bash -s -- $*"`, and the login shell on the host parses it again. Nothing is quoted for that shell (`printf %q` or equivalent). `remote.sh:41` checks the domain, and the relay checks the label, only after that parse. `provision 'x; sudo reboot'` runs `sudo reboot` beside the script. As root, `provision '$(reboot)'` runs in the same shell that is root. `accounts-add 'a b'` is split into two words before `bash -s` sees it. `release` and `install-cert` happen to pass only hex and script-built paths. `provision` and `accounts-*` take operator text.

**Non-blocking**

1. `relay/deploy/remote.sh:190` and `relay/deploy/deploy.sh:43`. A refused certificate hits `die` before `rm -f "$cert" "$key"` at line 195. There is no trap. The private key stays at `/tmp/relay-key.<unix second>`, a name chosen before the upload. `scp` keeps the source mode, so a `0600` key stays readable only by the uploader. A `0644` key is world-readable, and a symlink already at that path is followed. The success path does delete it. `accounts-add` prints the connector token on that SSH stdout and does not put it in the journal or in `argv`. `dns.env` is `0640` `root:tamber-relay`, which matches who must run the hook. systemd still copies it into the unit environment (`remote.sh:156`), and `systemctl show -p Environment` shows that to any local user. On this image those users are the operator and the service account.

2. `relay/deploy/remote.sh:67`. If `sshd -t` fails, the script does not reload, but the new drop-in is already in `sshd_config.d`. The running daemon keeps the old config. The next start of sshd (reboot, openssh upgrade) will refuse the config. On Ubuntu 22.04+ and Debian 12 the three lines are valid, so this is the failure path only. Firewall order is safe: `ufw --force reset` turns the firewall off, `22/tcp` is allowed, then `ufw --force enable`. A second run whose rules already match does not reset.

3. `relay/deploy/remote.sh:246`. `rollback` records the release it is leaving, switches, then `health || exit 1`. It does not switch back if that health check fails. `previous` then names the release you left, so another `rollback` returns to it. Accounts state is not on this path: `accounts.json` is created only when missing (`remote.sh:129`), and prune deletes release directories only.

4. `relay/deploy/remote.sh:180`. The unit is enabled even when `current` does not exist yet. A reboot after `provision` and before the first `release` starts `tamber-relay.service`, the binary is missing, and `Restart=on-failure` with `RestartSec=2` crash-loops it. The journal cap limits the disk use. It does not affect sshd.

5. `relay/deploy/deploy.sh:68`. `check` probes 80, 8080, 3000, 7740, and 8443, then prints that only 22 and 443 answer. It never checks 22, and it does not look at any other port.

6. `scripts/tamber-relay-gate.mjs:81`. Deployed mode still runs the seven lines against the real relay. The tap is a TCP proxy in front of `RELAY_GATE_RELAY` (default the domain on 443). Connector, TXT, and client sockets all use it, and the connector writes the bearer token only after `secureConnect`, so the capture is the same ciphertext the relay's public port sees. `captured.length > 0` and a cleartext search still mean what they meant on loopback. `bun test scripts/tamber-relay-server.test.mjs` passed (25 tests), including the env parser. That test never calls `gate({ deployed })`, so a break in the deployed branch would not fail the unit test.
