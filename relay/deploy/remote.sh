#!/usr/bin/env bash
# The relay host's side of relay/deploy/deploy.sh, run as root over SSH (`bash -s -- <command> ...`).
# Every command is idempotent: run it again and it changes only what differs. Ubuntu 22.04+ or Debian 12+.
#
#   provision <domain>          user, directories, firewall (22 and 443 only), sshd keys-only, journald cap,
#                               unattended upgrades, the systemd unit, the config; a placeholder certificate
#                               for <domain> when none is installed (marked; R4 replaces it by ACME)
#   release <sha256> <path>     install the uploaded binary at <path> as a release, switch to it, restart,
#                               health-check, and switch back to the previous release when that fails
#   rollback                    switch to the previous release and restart (health-checked)
#   health                      the service, its port, its own certificate by SNI, an unknown name closed
#   status                      releases, the current one, the unit, the firewall, the last log lines
#   install-cert <cert> <key>   the relay's own certificate (PEM, uploaded): checked to name the domain alone,
#                               valid now and the key's, installed, the placeholder mark removed, restarted
#   accounts-add <label>        a relay account; its connector token printed once, to this terminal only
#   accounts-revoke <label>     revoked for good
set -euo pipefail

APP=tamber-relay
OPT=/opt/$APP
ETC=/etc/$APP
VAR=/var/lib/$APP
UNIT=/etc/systemd/system/$APP.service
KEEP=5
say() { printf '%s: %s\n' "$APP" "$*"; }
die() { printf '%s: %s\n' "$APP" "$*" >&2; exit 1; }
[ "$(id -u)" = 0 ] || die "run as root (deploy.sh runs this through sudo when its user isn't root)"

# Writes stdin to $1 (mode $2, owner $3) only when it differs; says so when it does, and sets CHANGED.
CHANGED=0
put() {
  local path=$1 mode=$2 owner=$3 tmp
  tmp=$(mktemp "$path.XXXXXX")
  cat > "$tmp"
  if [ -f "$path" ] && cmp -s "$tmp" "$path"; then rm -f "$tmp"; else mv -f "$tmp" "$path"; say "wrote $path"; CHANGED=1; fi
  chmod "$mode" "$path"; chown "$owner" "$path"
}

provision() {
  local domain=${1:?provision <domain>}
  [[ $domain =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$ ]] || die "domain: lowercase and dotted, not $domain"
  . /etc/os-release
  case "$ID" in ubuntu|debian) ;; *) die "Ubuntu or Debian only (this is $PRETTY_NAME)";; esac

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
  install -d -m 0750 -o root -g $APP $ETC $ETC/tls
  install -d -m 0750 -o $APP -g $APP $VAR

  # SSH: keys only, root by key only. Checked before reloading, so a bad file never locks the host out.
  CHANGED=0
  put /etc/ssh/sshd_config.d/10-$APP.conf 0644 root:root <<'EOF'
# relay/deploy: keys only
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin prohibit-password
EOF
  if [ $CHANGED = 1 ]; then
    # (A drop-in sshd refuses is taken out again, so no later restart of sshd meets it.)
    sshd -t || { rm -f /etc/ssh/sshd_config.d/10-$APP.conf; die "sshd rejects its config: the drop-in removed, nothing reloaded"; }
    systemctl reload ssh 2>/dev/null || systemctl reload sshd
  fi

  # Firewall: in, only 22 and 443 (TCP, v4 and v6); out, anything (ACME, apt, the DNS provider's API).
  # Rebuilt only when the rules differ, so a rerun never opens a window. 22 is allowed before enabling.
  # sshd must listen on 22, the port the firewall keeps: one on another port would be shut out.
  local ports
  ports=$(sshd -T 2>/dev/null | awk '$1 == "port" {print $2}' || true)
  printf '%s\n' "$ports" | grep -qx 22 || die "sshd does not listen on port 22 (sshd -T: port ${ports:-none}): the firewall would shut it out; nothing changed"
  local want got
  want=$'22/tcp ALLOW IN Anywhere\n443/tcp ALLOW IN Anywhere\n22/tcp (v6) ALLOW IN Anywhere (v6)\n443/tcp (v6) ALLOW IN Anywhere (v6)'
  got=$(ufw status verbose 2>/dev/null | awk '/^--/{on=1;next} on && NF' | sed -E 's/ +/ /g; s/ $//' | sort)
  if ! ufw status | grep -q '^Status: active' || [ "$got" != "$(printf '%s\n' "$want" | sort)" ] || ! ufw status verbose | grep -q 'deny (incoming)'; then
    say "firewall: only 22/tcp and 443/tcp in"
    ufw --force reset >/dev/null
    ufw default deny incoming >/dev/null
    ufw default allow outgoing >/dev/null
    ufw allow 22/tcp >/dev/null
    ufw allow 443/tcp >/dev/null
    ufw --force enable >/dev/null
  fi

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

  # The relay's config. Accounts live in its state directory (labels.seen is written beside them).
  put $ETC/relay.json 0640 root:$APP <<EOF
{
  "domain": "$domain",
  "listen": "0.0.0.0:443",
  "tls": { "cert": "tls/cert.pem", "key": "tls/key.pem" },
  "accounts": "$VAR/accounts.json",
  "dns": { "command": "$ETC/dns-hook" }
}
EOF
  [ -f $VAR/accounts.json ] || put $VAR/accounts.json 0640 $APP:$APP <<<'{ "accounts": {}, "revoked": [] }'

  # Its certificate: one for the relay's own name alone. With none installed (or a placeholder for
  # another name), a self-signed placeholder so the service runs and can be checked; marked, and
  # refused by every connector (they verify WebPKI), until the ACME step installs the real one.
  if [ ! -f $ETC/tls/cert.pem ] || { [ -f $ETC/tls/PLACEHOLDER ] && [ "$(cat $ETC/tls/PLACEHOLDER)" != "$domain" ]; }; then
    say "certificate: a self-signed PLACEHOLDER for $domain (replace by ACME: runbook, 'Certificate')"
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 -subj "/CN=$domain" \
      -addext "subjectAltName=DNS:$domain" -keyout $ETC/tls/key.pem -out $ETC/tls/cert.pem 2>/dev/null
    printf '%s' "$domain" > $ETC/tls/PLACEHOLDER
    CHANGED=1
  fi
  chown root:$APP $ETC/tls/cert.pem $ETC/tls/key.pem; chmod 0640 $ETC/tls/cert.pem $ETC/tls/key.pem

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
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
RestrictSUIDSGID=true
LockPersonality=true
ReadWritePaths=$VAR
# (No MemoryDenyWriteExecute: Bun's JavaScript engine compiles to machine code.)

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  # (Enabled once a release exists: with none, a reboot would only crash-loop it.)
  if [ ! -e $OPT/current ]; then say "provisioned; no release yet (deploy.sh release)"
  elif [ $CHANGED = 1 ]; then systemctl enable $APP >/dev/null 2>&1; systemctl restart $APP || true; health || exit 1
  else systemctl enable $APP >/dev/null 2>&1; systemctl start $APP || true; health || exit 1; fi
}

install_cert() {
  local cert=${1:?install-cert <cert> <key>} key=${2:?install-cert <cert> <key>} domain san from
  # The uploads go whatever happens (deploy.sh puts them in a private directory of their own).
  trap "unload $(printf '%q ' "$cert" "$key")" EXIT
  domain=$(sed -nE 's/.*"domain": *"([^"]+)".*/\1/p' $ETC/relay.json)
  san=$(openssl x509 -in "$cert" -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' ' || true)
  [ "$san" = "DNS:$domain" ] || die "install-cert: the certificate names '${san:-nothing}', not $domain alone"
  openssl x509 -in "$cert" -noout -checkend 0 >/dev/null || die "install-cert: the certificate has expired"
  from=$(openssl x509 -in "$cert" -noout -startdate | cut -d= -f2)
  [ "$(date -d "$from" +%s)" -le "$(date +%s)" ] || die "install-cert: the certificate is not valid until $from"
  [ "$(openssl x509 -in "$cert" -noout -pubkey | sha256sum)" = "$(openssl pkey -in "$key" -pubout | sha256sum)" ] || die "install-cert: the key is not the certificate's"
  # The pair in place together, the old one kept beside it, and put back if the relay won't run on the new one.
  install -m 0640 -o root -g $APP "$cert" $ETC/tls/cert.pem.new
  install -m 0640 -o root -g $APP "$key" $ETC/tls/key.pem.new
  cp -p $ETC/tls/cert.pem $ETC/tls/cert.pem.old; cp -p $ETC/tls/key.pem $ETC/tls/key.pem.old
  [ -f $ETC/tls/PLACEHOLDER ] && mv -f $ETC/tls/PLACEHOLDER $ETC/tls/PLACEHOLDER.old
  mv -f $ETC/tls/key.pem.new $ETC/tls/key.pem && mv -f $ETC/tls/cert.pem.new $ETC/tls/cert.pem
  unload "$cert" "$key"
  say "certificate installed for $domain (from $from until $(openssl x509 -in $ETC/tls/cert.pem -noout -enddate | cut -d= -f2))"
  if [ -e $OPT/current ]; then
    systemctl restart $APP || true
    if ! health; then
      say "the relay did not run on the new certificate: the old one put back"
      mv -f $ETC/tls/key.pem.old $ETC/tls/key.pem; mv -f $ETC/tls/cert.pem.old $ETC/tls/cert.pem
      [ -f $ETC/tls/PLACEHOLDER.old ] && mv -f $ETC/tls/PLACEHOLDER.old $ETC/tls/PLACEHOLDER
      systemctl restart $APP || true; health || true
      exit 1
    fi
  fi
  rm -f $ETC/tls/cert.pem.old $ETC/tls/key.pem.old $ETC/tls/PLACEHOLDER.old
}

# An upload's private directory removed (deploy.sh makes one per upload, /tmp/tamber-relay-upload.*); a path
# anywhere else, only the file itself.
unload() { local f d; for f in "$@"; do d=$(dirname "$f"); case "$d" in /tmp/tamber-relay-upload.?*) rm -rf "$d";; *) rm -f "$f";; esac; done; }

current() { readlink $OPT/current 2>/dev/null | xargs -r basename; }

# Switches `current` to release $1 atomically, then restarts. A failed restart is the health check's to find,
# never an exit here, so a release or a rollback always reaches its switch back.
switch() {
  ln -sfn "$OPT/releases/$1" $OPT/current.new && mv -T $OPT/current.new $OPT/current
  systemctl restart $APP || say "restart failed (the health check says why)"
}

release() {
  local sha=${1:?release <sha256> <path>} path=${2:?release <sha256> <path>} id was before
  trap "unload $(printf '%q' "$path")" EXIT
  [ -f $UNIT ] || die "not provisioned (deploy.sh provision first)"
  [ "$(sha256sum "$path" | cut -d' ' -f1)" = "$sha" ] || die "the upload's digest is not $sha"
  id=${sha:0:16}
  # The binary must run here (its architecture, its libc) before anything switches to it.
  chmod 0755 "$path"
  "$path" --help >/dev/null || die "the new binary doesn't run on this host (architecture or libc): nothing switched"
  install -d -m 0755 $OPT/releases/$id
  install -m 0755 -o root -g root "$path" $OPT/releases/$id/$APP
  was=$(current || true); before=$(cat $OPT/previous 2>/dev/null || true)
  if [ "$was" = "$id" ] && systemctl is-active -q $APP; then say "release $id is already current and running"; health || exit 1; return; fi
  [ -n "$was" ] && [ "$was" != "$id" ] && printf '%s' "$was" > $OPT/previous
  say "switching ${was:-nothing} -> $id"
  switch "$id"
  if health; then
    # Keep the newest $KEEP releases, never the current or previous one.
    ls -1t $OPT/releases | tail -n +$((KEEP + 1)) | while read -r old; do
      [ "$old" = "$id" ] || [ "$old" = "$(cat $OPT/previous 2>/dev/null)" ] || { rm -rf "${OPT:?}/releases/$old"; say "pruned $old"; }
    done
  elif [ -n "$was" ] && [ "$was" != "$id" ]; then
    say "release $id failed its health check: back to $was (the failed release removed)"
    switch "$was"; health || true
    if [ -n "$before" ]; then printf '%s' "$before" > $OPT/previous; else rm -f $OPT/previous; fi
    rm -rf "${OPT:?}/releases/$id"
    exit 1
  else
    exit 1
  fi
}

rollback() {
  local to was
  to=$(cat $OPT/previous 2>/dev/null) || die "no previous release recorded"
  [ -x "$OPT/releases/$to/$APP" ] || die "the previous release $to is gone"
  was=$(current)
  say "rolling back $was -> $to"
  switch "$to"
  if health; then printf '%s' "$was" > $OPT/previous
  else
    say "release $to failed its health check: back to $was"
    switch "$was"; health || true
    exit 1
  fi
}

# The service runs and stays up, holds 443, answers its own name with its certificate, and closes an unknown
# name. Returns 1 (saying why) rather than exiting, so a release can switch back.
health() {
  local domain i pid subject
  bad() { printf '%s: health: %s\n' "$APP" "$*" >&2; journalctl -u $APP -n 10 --no-pager >&2; return 1; }
  domain=$(sed -nE 's/.*"domain": *"([^"]+)".*/\1/p' $ETC/relay.json)
  for i in $(seq 1 20); do
    systemctl is-active -q $APP && ss -Hltn 'sport = :443' | grep -q . && break
    sleep 0.5
  done
  # Up for two seconds by the same process: a crash loop is active between its restarts.
  pid=$(systemctl show -p MainPID --value $APP)
  sleep 2
  systemctl is-active -q $APP && [ "$pid" != 0 ] && [ "$(systemctl show -p MainPID --value $APP)" = "$pid" ] || { bad "the service is not running steadily"; return 1; }
  ss -Hltn 'sport = :443' | grep -q . || { bad "nothing listens on 443"; return 1; }
  # Each probe bounded (a release that holds a handshake open fails, not hangs), and read from its output, never
  # its exit status.
  local own unknown
  own=$(timeout 10 openssl s_client -connect 127.0.0.1:443 -servername "$domain" </dev/null 2>/dev/null || true)
  subject=$(printf '%s' "$own" | openssl x509 -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' ' || true)
  [ "$subject" = "DNS:$domain" ] || { bad "443 did not answer $domain with its certificate (got '${subject:-nothing}')"; return 1; }
  unknown=$(timeout 10 openssl s_client -connect 127.0.0.1:443 -servername "unknown.$domain.invalid" </dev/null 2>/dev/null || true)
  case "$unknown" in *'BEGIN CERTIFICATE'*) bad "an unknown name was answered with a certificate"; return 1;; esac
  say "health: OK (running $(current), 443 answers $domain, an unknown name is closed)"
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
