#!/usr/bin/env bash
# The relay host's side of relay/deploy/deploy.sh, run as root over SSH (`bash -s -- <command> ...`).
# Every command is idempotent: run it again and it changes only what differs. Ubuntu 22.04+ or Debian 12+.
# OUTWARD-FACING: it opens 443 to the internet. Only on a host Charlie provides for the relay (theme2, 2026-10-24).
#
#   provision <domain>          user, directories, firewall (22 and 443 only), sshd keys-only, journald cap,
#                               unattended upgrades, the systemd unit, the config; a placeholder certificate
#                               for <domain> when none is installed (marked; R4 replaces it by ACME)
#   release <sha256> <path>     install the uploaded binary at <path> as a release, switch to it, restart,
#                               health-check, and switch back to the previous release when that fails
#   rollback                    switch to the previous release and restart (health-checked, switched back on failure)
#   health                      the service, its port (v4 and v6), its own certificate by SNI, an unknown name closed
#   status                      releases, the current one, the unit, the firewall, the last log lines
#   install-cert <cert> <key>   the relay's own certificate (PEM, uploaded): checked to name the domain alone,
#                               valid now and the key's, switched in as a pair, the old pair back if the relay fails
#   accounts-add <label>        a relay account; its connector token printed once, to this terminal only
#   accounts-revoke <label>     revoked for good
set -euo pipefail
# (ufw and sshd print words this script reads: in English, whatever the host's locale)
export LC_ALL=C

APP=tamber-relay
OPT=/opt/$APP
ETC=/etc/$APP
VAR=/var/lib/$APP
UNIT=/etc/systemd/system/$APP.service
KEEP=5
say() { printf '%s: %s\n' "$APP" "$*"; }
die() { printf '%s: %s\n' "$APP" "$*" >&2; exit 1; }
[ "$(id -u)" = 0 ] || die "run as root (deploy.sh runs this through sudo when its user isn't root)"
# One change at a time on this host: provision, release, rollback, install-cert and accounts share links and state.
exec 9>/run/$APP-deploy.lock
flock -w 120 9 || die "another deploy.sh command holds this host (/run/$APP-deploy.lock) after 2 minutes"

# Writes stdin to $1 (mode $2, owner $3) only when it differs; says so when it does, and sets CHANGED.
CHANGED=0
put() {
  local path=$1 mode=$2 owner=$3 tmp
  tmp=$(mktemp "$path.XXXXXX")
  cat > "$tmp"
  if [ -f "$path" ] && cmp -s "$tmp" "$path"; then rm -f "$tmp"; else mv -f "$tmp" "$path"; say "wrote $path"; CHANGED=1; fi
  chmod "$mode" "$path"; chown "$owner" "$path"
}

# Points symlink $1 at $2 in one rename. Returns 1 when it couldn't (never half-done).
relink() { ln -sfn "$2" "$1.new" && mv -T "$1.new" "$1"; }

# An upload removed: its private directory, when it is one deploy.sh made (/tmp/tamber-relay-upload.<random>/<file>);
# any other path is left alone.
unload() {
  local f
  for f in "$@"; do
    if [[ $f =~ ^/tmp/tamber-relay-upload\.[A-Za-z0-9]+/[A-Za-z0-9._-]+$ ]]; then rm -rf "$(dirname "$f")"; else say "not removing $f: no upload of deploy.sh's"; fi
  done
}

domain_of() { sed -nE 's/.*"domain": *"([^"]+)".*/\1/p' $ETC/relay.json; }
current() { readlink $OPT/current 2>/dev/null | xargs -r basename; }

# The certificate pair in force: $ETC/tls, a link to one complete pair under tls.d/ (switched as a whole).
tls_new() { local d; d=$ETC/tls.d/$(date +%Y%m%d%H%M%S)-$RANDOM; install -d -m 0750 -o root -g $APP "$d"; printf '%s' "$d"; }
tls_prune() { local keep; keep=$(readlink -f $ETC/tls); for d in $ETC/tls.d/*; do [ "$d" = "$keep" ] || [ "$d" = "${1:-}" ] || rm -rf "$d"; done; }

# Deletes every inbound rule but 22/tcp and 443/tcp, one at a time (rules show only while ufw is active). A status
# ufw can't give stops the run, never reads as "nothing to delete".
KEEP_RULE='^[0-9]+ (22|443)/tcp( \(v6\))? ALLOW IN Anywhere( \(v6\))?$'
rules() { local out; out=$(ufw status numbered) || die "firewall: ufw status failed"; printf '%s\n' "$out" | sed -nE 's/^\[ *([0-9]+)\] (.*)$/\1 \2/p' | sed -E 's/ +/ /g; s/ $//'; }
prune_rules() {
  local all n
  for _ in $(seq 1 64); do
    all=$(rules)
    n=$(printf '%s\n' "$all" | grep -vE "$KEEP_RULE" | awk 'NF && NR >= 1 {print $1; exit}' || true)
    [ -n "$n" ] || return 0
    say "firewall: deleting inbound rule $n ($(printf '%s\n' "$all" | awk -v n="$n" '$1 == n {$1 = ""; print substr($0, 2)}'))"
    ufw --force delete "$n" >/dev/null || die "firewall: could not delete rule $n"
  done
  die "firewall: rules kept coming back after 64 deletions"
}

provision() {
  local domain=${1:?provision <domain>}
  [[ $domain =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$ ]] || die "domain: lowercase and dotted, not $domain"
  . /etc/os-release
  case "$ID" in ubuntu|debian) ;; *) die "Ubuntu or Debian only (this is $PRETTY_NAME)";; esac
  # (systemd 249 or later: older ones ignore the unit's SocketBindAllow, and the relay could bind any low port)
  case "$ID:${VERSION_ID%%.*}" in ubuntu:2[2-9]|ubuntu:[3-9]?|debian:1[2-9]) ;; *) die "Ubuntu 22.04+ or Debian 12+ only (this is $PRETTY_NAME)";; esac

  # SSH first, before anything changes: the firewall keeps 22 alone, so sshd must be what answers on 22, and this
  # very connection must have come in on 22 (deploy.sh passes the port it reached, RELAY_SSH_SERVER_PORT).
  local ports
  ports=$(sshd -T 2>/dev/null | awk '$1 == "port" {print $2}' || true)
  printf '%s\n' "$ports" | grep -qx 22 || die "sshd is not configured on port 22 (sshd -T: port ${ports:-none}): the firewall would shut it out; nothing changed"
  [ -n "$(ss -Hltn 'sport = :22' || true)" ] || die "nothing listens on port 22: the firewall would shut SSH out; nothing changed"
  local conn=(${RELAY_SSH_CONNECTION:-})
  [ ${#conn[@]} = 4 ] || die "provision needs the SSH connection it runs over (deploy.sh passes it as RELAY_SSH_CONNECTION); run it through deploy.sh"
  [ "${conn[3]}" = 22 ] || die "this SSH connection came in on port ${conn[3]}, not 22: the firewall would shut it out; nothing changed"

  # Packages: the firewall, openssl (the placeholder certificate and health checks), security updates.
  local need=()
  for p in ufw openssl unattended-upgrades ca-certificates; do dpkg -s "$p" >/dev/null 2>&1 || need+=("$p"); done
  if [ ${#need[@]} -gt 0 ]; then
    say "installing ${need[*]}"
    DEBIAN_FRONTEND=noninteractive apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${need[@]}" >/dev/null
  fi
  put /etc/apt/apt.conf.d/20auto-upgrades 0644 root:root <<'EOF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
EOF

  # The service's user: no login, no home of its own but its state directory.
  getent group $APP >/dev/null || groupadd --system $APP
  id -u $APP >/dev/null 2>&1 || useradd --system --gid $APP --home-dir $VAR --no-create-home --shell /usr/sbin/nologin $APP
  install -d -m 0755 -o root -g root $OPT $OPT/releases
  install -d -m 0750 -o root -g $APP $ETC $ETC/tls.d
  install -d -m 0750 -o $APP -g $APP $VAR

  # SSH: keys only, root by key only. Tested before reloading (a drop-in sshd rejects is taken out again), then the
  # settings sshd will actually use are checked: an earlier file or line wins in sshd, so a drop-in alone proves little.
  CHANGED=0
  put /etc/ssh/sshd_config.d/10-$APP.conf 0644 root:root <<'EOF'
# relay/deploy: keys only
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin prohibit-password
PubkeyAuthentication yes
EOF
  if [ $CHANGED = 1 ]; then
    sshd -t || { rm -f /etc/ssh/sshd_config.d/10-$APP.conf; die "sshd rejects its config: the drop-in removed, nothing reloaded"; }
    systemctl reload ssh 2>/dev/null || systemctl reload sshd
  fi
  # A Match block can turn passwords back on for some connections, which no single sshd -T shows, and sshd's syntax
  # for one (quotes, keyword=value, escapes, an Include anywhere) is more than a scan can follow. So none is allowed,
  # and sshd itself says which it read: `sshd -ddd -t` logs every Match line it checks, in every file it includes
  # (stock Ubuntu and Debian images have none). The settings then hold for every connection, checked below as sshd
  # evaluates them for this very one.
  local dbg
  dbg=$(sshd -ddd -t 2>&1) || die "sshd -t rejects its config: ${dbg##*$'\n'}"
  # (string tests, not a pipe: under pipefail a grep that stops early can make printf's SIGPIPE read as no match)
  [[ $dbg == debug* || $dbg == *$'\n'debug* ]] || die "sshd -ddd -t gave no debug output to check for Match blocks; nothing changed past the drop-in"
  # OpenSSH logs "checking syntax for '<criteria>'" for each Match it parses (9.x writes 'Match <criteria>', 8.9 the
  # criteria alone), and for nothing else.
  if [[ $dbg == *"checking syntax for '"* ]]; then
    local m=${dbg#*checking syntax for \'}; m=${m%%\'*}
    die "sshd's config has a Match block ('$m'): provision allows none, as it can turn password login back on for some connections; remove it, then provision again"
  fi
  local who eff
  for who in root "${RELAY_SSH_USER:-root}"; do
    eff=$(sshd -T -C "user=$who,host=${conn[0]},addr=${conn[0]},laddr=${conn[2]},lport=${conn[3]}" 2>/dev/null) || die "sshd -T -C failed for $who from ${conn[0]}"
    for want in 'passwordauthentication no' 'kbdinteractiveauthentication no' 'pubkeyauthentication yes'; do
      [[ $'\n'$eff$'\n' == *$'\n'"$want"$'\n'* ]] || die "sshd's effective config for $who from ${conn[0]} is not '$want' (an earlier setting wins over the drop-in): fix it in /etc/ssh, then provision again"
    done
    [[ $'\n'$eff$'\n' =~ $'\n'permitrootlogin\ (prohibit-password|without-password|no)$'\n' ]] || die "sshd's effective config lets root log in by password: fix it in /etc/ssh, then provision again"
  done

  # Firewall: in, only 22 and 443 (TCP, v4 and v6); out, anything (ACME, apt, the DNS provider's API). Never reset or
  # disabled on the way: 22 and 443 allowed first, any other inbound rule deleted one by one, then enabled.
  CHANGED=0
  if grep -q '^IPV6=' /etc/default/ufw && ! grep -qx 'IPV6=yes' /etc/default/ufw; then sed -i 's/^IPV6=.*/IPV6=yes/' /etc/default/ufw; CHANGED=1; say "firewall: IPv6 filtered too"; fi
  # (22 allowed before anything can deny it)
  ufw allow 22/tcp >/dev/null || die "firewall: could not allow 22/tcp"
  ufw allow 443/tcp >/dev/null || die "firewall: could not allow 443/tcp"
  ufw default deny incoming >/dev/null
  ufw default allow outgoing >/dev/null
  prune_rules
  if ! ufw status | grep -q '^Status: active'; then say "firewall: enabled, only 22/tcp and 443/tcp in"; ufw --force enable >/dev/null; prune_rules
  elif [ $CHANGED = 1 ]; then ufw reload >/dev/null; fi
  ufw allow 22/tcp >/dev/null; ufw allow 443/tcp >/dev/null
  # What it ends as, checked: active, deny in by default, and exactly 22 and 443 on both families.
  local got
  got=$(rules | sed -E 's/^[0-9]+ //' | sort)
  ufw status verbose | grep -q '^Status: active' && ufw status verbose | grep -q 'deny (incoming)' \
    && [ "$got" = "$(printf '%s\n' '22/tcp ALLOW IN Anywhere' '22/tcp (v6) ALLOW IN Anywhere (v6)' '443/tcp ALLOW IN Anywhere' '443/tcp (v6) ALLOW IN Anywhere (v6)' | sort)" ] \
    || die "firewall: not as wanted after the change (rules: $(printf '%s' "$got" | tr '\n' ';'))"

  # Logs: the journal, capped (the relay logs one line an event to stdout, never a token).
  install -d -m 0755 /etc/systemd/journald.conf.d
  CHANGED=0
  put /etc/systemd/journald.conf.d/$APP.conf 0644 root:root <<'EOF'
[Journal]
SystemMaxUse=500M
SystemMaxFileSize=50M
MaxRetentionSec=30day
EOF
  [ $CHANGED = 1 ] && systemctl restart systemd-journald

  # From here, a change restarts the relay (its config, certificate and unit are read at start).
  CHANGED=0

  # The DNS hook the relay runs for an account's ACME TXT record. A placeholder that refuses until the
  # provider is chosen (R4, waits on the domain): it is replaced by the provider's script, its API token in dns.env.
  if [ ! -f $ETC/dns-hook ]; then
    put $ETC/dns-hook 0750 root:$APP <<'EOF'
#!/bin/sh
# PLACEHOLDER (relay/deploy): no DNS provider yet. Replace with the provider's script (runbook, "DNS").
echo "tamber-relay dns-hook: no DNS provider is configured on this relay yet" >&2
exit 1
EOF
  fi
  [ -f $ETC/dns.env ] || put $ETC/dns.env 0640 root:$APP < /dev/null
  chown root:$APP $ETC/dns-hook $ETC/dns.env; chmod 0750 $ETC/dns-hook; chmod 0640 $ETC/dns.env

  # The relay's config. Both address families on 443. Accounts live in its state directory (labels.seen beside them).
  put $ETC/relay.json 0640 root:$APP <<EOF
{
  "domain": "$domain",
  "listen": "[::]:443",
  "tls": { "cert": "tls/cert.pem", "key": "tls/key.pem" },
  "accounts": "$VAR/accounts.json",
  "dns": { "command": "$ETC/dns-hook" }
}
EOF
  # (made by the service's user: root never writes, chmods or chowns through a path in the relay's own directory)
  [ -e $VAR/accounts.json ] || runuser -u $APP -- sh -c 'umask 027 && cd "$1" && printf "%s\n" "{ \"accounts\": {}, \"revoked\": [] }" > accounts.json.new && mv -n accounts.json.new accounts.json; rm -f accounts.json.new' sh "$VAR"
  [ -f $VAR/accounts.json ] || die "could not make $VAR/accounts.json"

  # Its certificate: one for the relay's own name alone, a pair under tls.d/ that $ETC/tls links to. With none
  # installed (or a placeholder for another name), a self-signed placeholder so the service runs and can be checked;
  # marked, and refused by every connector (they verify WebPKI), until the ACME step installs the real one.
  if [ -d $ETC/tls ] && [ ! -L $ETC/tls ]; then local was; was=$(tls_new); mv $ETC/tls/* "$was"/ 2>/dev/null || true; rmdir $ETC/tls; relink $ETC/tls "$was"; fi
  if [ ! -f $ETC/tls/cert.pem ] || { [ -f $ETC/tls/PLACEHOLDER ] && [ "$(cat $ETC/tls/PLACEHOLDER)" != "$domain" ]; }; then
    say "certificate: a self-signed PLACEHOLDER for $domain (replace by ACME: runbook, 'Certificate')"
    local d; d=$(tls_new)
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 -subj "/CN=$domain" \
      -addext "subjectAltName=DNS:$domain" -keyout "$d/key.pem" -out "$d/cert.pem" 2>/dev/null
    printf '%s' "$domain" > "$d/PLACEHOLDER"
    chown root:$APP "$d"/*; chmod 0640 "$d"/*
    relink $ETC/tls "$d" || die "could not switch the certificate"
    tls_prune
    CHANGED=1
  fi

  put $UNIT 0644 root:root <<EOF
[Unit]
Description=tamber relay (SNI pass-through for tamber serve --relay)
Documentation=https://github.com/ccheever/exact2/blob/main/docs/tamber-relay-service.md
After=network-online.target
Wants=network-online.target

[Service]
User=$APP
Group=$APP
WorkingDirectory=$VAR
ExecStart=$OPT/current/$APP --config $ETC/relay.json
ExecReload=/bin/kill -HUP \$MAINPID
EnvironmentFile=-$ETC/dns.env
Restart=on-failure
RestartSec=2
LimitNOFILE=65536
MemoryMax=1G
TasksMax=512
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
# 443 is the only port it binds; the cloud's metadata service and link-local are out of its reach.
SocketBindAllow=tcp:443
SocketBindDeny=any
IPAddressDeny=169.254.0.0/16 fe80::/10 ::ffff:169.254.0.0/112 fd00:ec2::254
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectKernelLogs=true
ProtectControlGroups=true
ProtectClock=true
ProtectHostname=true
RestrictNamespaces=true
RestrictSUIDSGID=true
RestrictRealtime=true
LockPersonality=true
ReadWritePaths=$VAR
# (No MemoryDenyWriteExecute: Bun's JavaScript engine compiles to machine code.)

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  # (Enabled by the first release that passes its health check: with none, a reboot would only crash-loop it.)
  if [ ! -e $OPT/current ]; then say "provisioned; no release yet (deploy.sh release)"
  elif [ $CHANGED = 1 ]; then systemctl restart $APP || true; health || exit 1; systemctl enable -q $APP
  else systemctl start $APP || true; health || exit 1; systemctl enable -q $APP; fi
}

install_cert() {
  local cert=${1:?install-cert <cert> <key>} key=${2:?install-cert <cert> <key>} domain san from d old
  trap "unload $(printf '%q ' "$cert" "$key")" EXIT
  domain=$(domain_of)
  san=$(openssl x509 -in "$cert" -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' ' || true)
  [ "$san" = "DNS:$domain" ] || die "install-cert: the certificate names '${san:-nothing}', not $domain alone"
  openssl x509 -in "$cert" -noout -checkend 0 >/dev/null || die "install-cert: the certificate has expired"
  from=$(openssl x509 -in "$cert" -noout -startdate | cut -d= -f2)
  [ "$(date -d "$from" +%s)" -le "$(date +%s)" ] || die "install-cert: the certificate is not valid until $from"
  local a b
  a=$(openssl x509 -in "$cert" -noout -pubkey) || die "install-cert: the certificate can't be read"
  b=$(openssl pkey -in "$key" -pubout) || die "install-cert: the key can't be read"
  [ -n "$a" ] && [ "$a" = "$b" ] || die "install-cert: the key is not the certificate's"
  # The pair in a directory of its own, switched in by one rename; the old pair's directory kept until the relay
  # runs on the new one, and switched back to if it doesn't.
  d=$(tls_new)
  install -m 0640 -o root -g $APP "$cert" "$d/cert.pem"
  install -m 0640 -o root -g $APP "$key" "$d/key.pem"
  unload "$cert" "$key"
  old=$(readlink -f $ETC/tls)
  relink $ETC/tls "$d" || { rm -rf "$d"; die "install-cert: could not switch the certificate; the old one stays"; }
  say "certificate installed for $domain (from $from until $(openssl x509 -in "$d/cert.pem" -noout -enddate | cut -d= -f2))"
  if [ -e $OPT/current ]; then
    systemctl restart $APP || true
    if ! health; then
      say "the relay did not run on the new certificate: the old one switched back"
      relink $ETC/tls "$old" || die "install-cert: could not switch the old certificate back: $ETC/tls -> $d still"
      rm -rf "$d"
      systemctl restart $APP || true; health || true
      exit 1
    fi
  fi
  tls_prune
}

# Switches `current` to release $1 atomically, then restarts. Returns 1 when the link couldn't be switched; a failed
# restart is the health check's to find, never an exit here, so a release or a rollback always reaches its switch back.
switch() {
  relink $OPT/current "$OPT/releases/$1" || { say "could not point current at $1"; return 1; }
  systemctl restart $APP || say "restart failed (the health check says why)"
}

release() {
  local sha=${1:?release <sha256> <path>} path=${2:?release <sha256> <path>} id was before bin
  trap "unload $(printf '%q' "$path")" EXIT
  [ -f $UNIT ] || die "not provisioned (deploy.sh provision first)"
  [ "$(sha256sum "$path" | cut -d' ' -f1)" = "$sha" ] || die "the upload's digest is not $sha"
  id=${sha:0:16}; bin=$OPT/releases/$id/$APP
  was=$(current || true); before=$(cat $OPT/previous 2>/dev/null || true)
  if [ "$was" = "$id" ]; then
    # Already current: started if it isn't, never cleaned up as a first release (its recovery state stays).
    systemctl is-active -q $APP && say "release $id is already current and running" || { say "release $id is current but not running: starting it"; systemctl restart $APP || true; }
    health || die "the current release $id fails its health check: rollback, or release another build"
    systemctl enable -q $APP; return
  fi
  install -d -m 0755 -o root -g root $OPT/releases/$id
  install -m 0755 -o root -g root "$path" "$bin"
  # It must run here (its architecture, its libc) before anything switches to it: as the service's user, sandboxed
  # and with no network, never as root.
  if ! systemd-run --quiet --wait --pipe --collect -p User=$APP -p NoNewPrivileges=yes -p ProtectSystem=strict \
      -p ProtectHome=yes -p PrivateTmp=yes -p PrivateDevices=yes -p PrivateNetwork=yes -p PrivateIPC=yes \
      -p InaccessiblePaths=$ETC -p RuntimeMaxSec=30 -p MemoryMax=512M -p TasksMax=64 "$bin" --help >/dev/null; then
    [ "$was" = "$id" ] || rm -rf "${OPT:?}/releases/$id"
    die "the new binary doesn't run on this host (architecture or libc): nothing switched"
  fi
  [ -n "$was" ] && [ "$was" != "$id" ] && printf '%s' "$was" > $OPT/previous
  say "switching ${was:-nothing} -> $id"
  switch "$id" || die "nothing switched"
  if health; then
    systemctl enable -q $APP
    # Keep the newest $KEEP releases, never the current or previous one.
    ls -1t $OPT/releases | tail -n +$((KEEP + 1)) | while read -r old; do
      [ "$old" = "$id" ] || [ "$old" = "$(cat $OPT/previous 2>/dev/null)" ] || { rm -rf "${OPT:?}/releases/$old"; say "pruned $old"; }
    done
  elif [ -n "$was" ] && [ "$was" != "$id" ]; then
    say "release $id failed its health check: back to $was"
    switch "$was" || die "could not switch back to $was: current is still $id (run rollback)"
    health || true
    if [ -n "$before" ]; then printf '%s' "$before" > $OPT/previous; else rm -f $OPT/previous; fi
    [ "$(current)" = "$was" ] && rm -rf "${OPT:?}/releases/$id" && say "the failed release removed"
    exit 1
  else
    # The first release, with nothing to go back to: stopped, not left crash-looping.
    say "the first release $id failed its health check: the relay stopped and the release removed"
    systemctl stop $APP || true; systemctl disable -q $APP 2>/dev/null || true
    rm -f $OPT/current; rm -rf "${OPT:?}/releases/$id"
    exit 1
  fi
}

rollback() {
  local to was
  to=$(cat $OPT/previous 2>/dev/null) || die "no previous release recorded"
  [ -x "$OPT/releases/$to/$APP" ] || die "the previous release $to is gone"
  was=$(current)
  say "rolling back $was -> $to"
  switch "$to" || die "nothing switched"
  if health; then printf '%s' "$was" > $OPT/previous
  else
    say "release $to failed its health check: back to $was"
    switch "$was" || die "could not switch back to $was: current is $to"
    health || true
    exit 1
  fi
}

# One TLS probe of 127.0.0.1 or ::1 by SNI, bounded: sets PROBE (its output) and PROBE_RC (124 when it timed out).
probe() { PROBE=$(timeout 10 openssl s_client -connect "$1" -servername "$2" </dev/null 2>/dev/null) && PROBE_RC=0 || PROBE_RC=$?; }

# The service runs and stays up, holds 443 (v4 and v6), answers its own name with its certificate, and closes an
# unknown name without holding it open. Returns 1 (saying why) rather than exiting, so a release can switch back.
health() {
  local domain i pid subject at
  bad() { printf '%s: health: %s\n' "$APP" "$*" >&2; journalctl -u $APP -n 10 --no-pager >&2; return 1; }
  domain=$(domain_of)
  for i in $(seq 1 20); do
    systemctl is-active -q $APP && [ -n "$(ss -Hltn 'sport = :443' || true)" ] && break
    sleep 0.5
  done
  # Up for two seconds by the same process: a crash loop is active between its restarts.
  pid=$(systemctl show -p MainPID --value $APP)
  sleep 2
  systemctl is-active -q $APP && [ "$pid" != 0 ] && [ "$(systemctl show -p MainPID --value $APP)" = "$pid" ] || { bad "the service is not running steadily"; return 1; }
  [ -n "$(ss -Hltn 'sport = :443' || true)" ] || { bad "nothing listens on 443"; return 1; }
  local addrs=(127.0.0.1:443)
  [ -f /proc/net/if_inet6 ] && addrs+=('[::1]:443')
  for at in "${addrs[@]}"; do
    probe "$at" "$domain"
    [ "$PROBE_RC" != 124 ] || { bad "$at held the handshake for $domain open"; return 1; }
    subject=$(printf '%s' "$PROBE" | openssl x509 -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' ' || true)
    [ "$subject" = "DNS:$domain" ] || { bad "$at did not answer $domain with its certificate (got '${subject:-nothing}')"; return 1; }
    probe "$at" "unknown.$domain.invalid"
    [ "$PROBE_RC" != 124 ] || { bad "$at held an unknown name open instead of closing it"; return 1; }
    case "$PROBE" in *'BEGIN CERTIFICATE'*) bad "$at answered an unknown name with a certificate"; return 1;; esac
  done
  say "health: OK (running $(current), ${addrs[*]} answer $domain, an unknown name is closed)"
  [ -f $ETC/tls/PLACEHOLDER ] && say "health: the certificate is a PLACEHOLDER (self-signed): connectors refuse it until the ACME step"
  return 0
}

status() {
  say "current: $(current || echo none); previous: $(cat $OPT/previous 2>/dev/null || echo none)"
  say "releases: $(ls -1t $OPT/releases 2>/dev/null | tr '\n' ' ')"
  systemctl --no-pager --lines=0 status $APP || true
  ufw status verbose
  journalctl -u $APP -n 20 --no-pager
}

accounts() {
  local action=$1 label=${2:?accounts-$1 <label>}
  [ -e $OPT/current ] || die "no release yet"
  # As the service's user, so labels.seen and the accounts file stay its own; the token goes to this terminal only.
  runuser -u $APP -- $OPT/current/$APP accounts "$action" "$label" --config $ETC/relay.json
}

cmd=${1:-}; shift || true
case "$cmd" in
  provision) provision "$@";;
  release) release "$@";;
  rollback) rollback;;
  health) health || exit 1;;
  status) status;;
  install-cert) install_cert "$@";;
  accounts-add) accounts add "$@";;
  accounts-revoke) accounts revoke "$@";;
  *) die "commands: provision <domain> | release <sha256> <path> | rollback | health | status | install-cert <cert> <key> | accounts-add <label> | accounts-revoke <label>";;
esac
