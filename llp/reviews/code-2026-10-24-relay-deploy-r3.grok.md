# Code review: relay deploy (R4 prep), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c5ad89725e`.
- **Method:** one brief (sha256 `ce37cc9d5c981a8199da6a8f72d651e524456692bda5f3576efff5663a69ebc8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `relay/deploy/deploy.sh:62` and `relay/deploy/deploy.sh:118`. `check` reports an unknown name closed when the probe is killed by its 10s alarm. `probe` is `perl -e 'alarm 10; exec @ARGV' … || true`, and the `case` only looks for `BEGIN CERTIFICATE`. An alarm kills `openssl` with status 142 and an empty capture, and that branch prints "an unknown name is closed" and leaves `ok=1`. A handshake still open at 10s is the case `health` now rejects with status 124. Keep the alarm status (exit 124 from a `SIGALRM` handler around `system`, not `exec`), and fail `check` on 124 the same way `health` does.

2. `relay/deploy/remote.sh:62` and `relay/deploy/remote.sh:130`. `prune_rules` keeps a rule only when the numbered status line matches the English text `ALLOW IN Anywhere`. `ufw` translates "Anywhere". On a non-English locale the `22/tcp` and `443/tcp` rules just added fail that regex and are deleted, and nothing adds them back. `ufw default deny incoming` has already run, so a new SSH connection is refused. `export LC_ALL=C` for every `ufw` command, allow 22/tcp and 443/tcp before the default-policy change, and allow both again after `prune_rules`.

3. `relay/deploy/remote.sh:217`. `IPAddressDeny=169.254.0.0/16 fe80::/10` does not cover an `AF_INET6` connect to `::ffff:169.254.169.254`. That address is not in the IPv4 list, and with `bindv6only=0` Linux delivers it to `169.254.169.254`. `fd00:ec2::254` (EC2's IPv6 metadata address) is not listed either. Add `::ffff:169.254.0.0/112` and `fd00:ec2::254` to `IPAddressDeny`.

**Non-blocking**

1. `relay/deploy/deploy.sh:37`. `StrictHostKeyChecking=yes` still consults `/etc/ssh/ssh_known_hosts`. A key or `@cert-authority` there is accepted beside the pinned line. Pass `-o GlobalKnownHostsFile=/dev/null` and `-o UpdateHostKeys=no`.

2. `relay/deploy/remote.sh:120`. `sshd -T` without `-C` shows the global config. A `Match` block can set `PasswordAuthentication yes` for the real session while the check sees `no`. Run `sshd -T -C` with this connection's user, address, and port from `SSH_CONNECTION`.

3. `scripts/tamber-relay-gate.mjs:176`. Line 3 treats any increase of the process-wide `forwarded` counter as "this plain connection reached the relay". A connector reconnect during the 10s probe satisfies it when this socket's outbound never connected. Set a flag on that socket in the tap's connect callback and require that flag.

4. `relay/deploy/remote.sh:301`. The smoke run is `User=tamber-relay` with `NoNewPrivileges`, `ProtectSystem=strict`, and `PrivateNetwork`. It is not root. That user can read `dns.env` and the TLS key (both `root:tamber-relay` mode `0640`) and can write `/dev/shm`, which stays world-readable. Give the smoke unit `PrivateIPC=yes` (or a read-only `/dev/shm`) and the same `IPAddressDeny` as the service.

5. `relay/deploy/remote.sh:73`. The host check is `ID` of `ubuntu` or `debian` only. Ubuntu 20.04 and Debian 11 start the unit while ignoring `SocketBindAllow=` (systemd 249+), so `CAP_NET_BIND_SERVICE` can bind any low port. Refuse versions below 22.04 and 12.

6. `relay/deploy/remote.sh:81`. An empty `RELAY_SSH_SERVER_PORT` skips the "this connection came in on 22" check. `deploy.sh` always sets it; a hand-run `remote.sh provision` does not. Require the variable.

7. `relay/deploy/remote.sh:48`. `unload` does `rm -f` on any path that is not an upload directory. A bad argument is deleted on the way out. Refuse a path that does not match `/tmp/tamber-relay-upload.<random>/<file>`.
