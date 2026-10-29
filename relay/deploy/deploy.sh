#!/usr/bin/env bash
# Deploys the tamber relay (scripts/tamber-relay-server.mjs, docs/tamber-relay-service.md) to one Ubuntu or
# Debian host over SSH, from a checkout on the developer's machine. Idempotent; the runbook is
# ~/x2theme/r4-deploy-runbook.md (LLP 1121 §5 10c R4).
#
# OUTWARD-FACING: the relay takes connections from the internet on 443. Run this only against a host Charlie
# provides for it (theme2, 2026-10-24); never against a machine of anyone else's, and never a second relay.
#
#   relay/deploy/deploy.sh <host> trust <SHA256:fingerprint>   the host's SSH key pinned, once, before anything else:
#                                                     the fingerprint from the provider's console, never from the
#                                                     host itself; every later connection refuses any other key
#   relay/deploy/deploy.sh <host> provision <domain>   the host made ready (relay/deploy/remote.sh provision)
#   relay/deploy/deploy.sh <host> release              build the relay for the host's architecture, upload, switch, health-check
#   relay/deploy/deploy.sh <host> rollback|health|status
#   relay/deploy/deploy.sh <host> install-cert <cert.pem> <key.pem>   the relay's own certificate (the ACME step's)
#   relay/deploy/deploy.sh <host> accounts-add <label>      its token printed once, here: never paste it into chat
#   relay/deploy/deploy.sh <host> accounts-revoke <label>
#   relay/deploy/deploy.sh <host> check <domain>       from here: 443 answers <domain>, an unknown name is closed,
#                                                     ssh answers, and common other ports are closed
#
# <host> is [user@]address (root when no user is named). Environment: RELAY_SSH_KEY (default
# ~/.ssh/tamber_relay_vps), RELAY_SSH_PORT (22), RELAY_KNOWN_HOSTS (default ~/.ssh/known_hosts, where `trust` pins the
# key), RELAY_CHECK_PORT (443, where check reaches the relay: a test VM's forwarded port). A user other than root runs
# the remote side through sudo.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
host=${1:?usage: deploy.sh <[user@]host> <trust <SHA256:…>|provision <domain>|release|rollback|health|status|install-cert <cert> <key>|accounts-add <label>|accounts-revoke <label>|check <domain>>}
cmd=${2:?a command}
shift 2
key_file=${RELAY_SSH_KEY:-$HOME/.ssh/tamber_relay_vps}
port=${RELAY_SSH_PORT:-22}
known=${RELAY_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}
# The host's key must already be pinned (`trust`): no first-use acceptance, so no one between here and the host can
# slip their own key in on the connection that carries the certificate key and the account tokens.
ssh_opts=(-i "$key_file" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile="$known" -o GlobalKnownHostsFile=/dev/null -o UpdateHostKeys=no -o ConnectTimeout=15)
# (Always a user named: without one, ssh's own choice could skip the sudo a non-root user needs.)
[[ $host == *@* ]] || host=root@$host
user=${host%%@*}
sudo=""; [ "$user" = root ] || sudo="sudo"
address=${host#*@}
# (how known_hosts names it: the address, or [address]:port off 22)
pinned_as=$address; [ "$port" = 22 ] || pinned_as="[$address]:$port"

# Every argument checked here, then quoted for the remote shell (which parses the command line again). The port this
# connection reached on the host goes with it, so provision can refuse a firewall that would shut it out.
DOMAIN_RE='^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$'
LABEL_RE='^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'
remote() { ssh "${ssh_opts[@]}" -p "$port" "$host" "$sudo env RELAY_SSH_CONNECTION=\"\$SSH_CONNECTION\" RELAY_SSH_USER=\"\$(id -un)\" bash -s -- $(printf '%q ' "$@")" < "$here/remote.sh"; }
# A private directory on the host for one upload (0700, this user's), removed from here too if the remote side never
# gets to it (a failed copy, an interrupted run): what's put in it is read by no one else and outlives nothing.
UPLOAD=""
cleanup() { [ -z "$UPLOAD" ] || ssh "${ssh_opts[@]}" -p "$port" "$host" "rm -rf $(printf '%q' "$UPLOAD")" < /dev/null || true; [ -z "${out:-}" ] || rm -rf "$out"; }
trap cleanup EXIT
updir() {
  local got
  got=$(ssh "${ssh_opts[@]}" -p "$port" "$host" 'umask 077; mktemp -d /tmp/tamber-relay-upload.XXXXXX' < /dev/null) || { echo "deploy.sh: the host made no upload directory" >&2; exit 1; }
  got=$(printf '%s\n' "$got" | tail -1)
  # (the cleanup's target only once it is what was asked for)
  [[ $got =~ ^/tmp/tamber-relay-upload\.[A-Za-z0-9]+$ ]] || { echo "deploy.sh: the host gave no upload directory (got '$got')" >&2; exit 1; }
  UPLOAD=$got
}
put_up() { scp -q "${ssh_opts[@]}" -P "$port" "$1" "$host:$2"; }
# A bounded TLS probe from here (macOS has no timeout(1)): its output, and HELD=1 when it was still open at 10 s.
probe() {
  PROBE=$(perl -e '$p = fork // die; if (!$p) { exec @ARGV } $SIG{ALRM} = sub { kill 9, $p; waitpid $p, 0; exit 124 }; alarm 10; waitpid $p, 0; exit($? >> 8)' \
    openssl s_client -connect "$address:${RELAY_CHECK_PORT:-443}" -servername "$1" </dev/null 2>/dev/null) && HELD=0 || { [ $? = 124 ] && HELD=1 || HELD=0; }
}
# The DNS names a PEM certificate on stdin carries, as `DNS:a,DNS:b` (OpenSSL's and LibreSSL's -text alike).
sans() { openssl x509 -noout -text 2>/dev/null | grep -A1 'Subject Alternative Name' | tail -1 | tr -d ' ' || true; }

case "$cmd" in
  trust)
    want=${1:?trust <SHA256:fingerprint, from the provider console>}
    keys=$(ssh-keyscan -p "$port" -T 15 "$address" 2>/dev/null || true)
    [ -n "$keys" ] || { echo "deploy.sh: no SSH host key from $address:$port" >&2; exit 1; }
    match=""
    while read -r line; do
      [ -n "$line" ] && [ "${line#\#}" = "$line" ] || continue
      [ "$(printf '%s\n' "$line" | ssh-keygen -lf - | awk '{print $2}')" = "$want" ] && match=$line
    done <<< "$keys"
    if [ -z "$match" ]; then
      echo "deploy.sh: no host key of $address matches $want. It offered:" >&2
      printf '%s\n' "$keys" | grep -v '^#' | ssh-keygen -lf - >&2
      exit 1
    fi
    mkdir -p "$(dirname "$known")"; touch "$known"
    if ssh-keygen -F "$pinned_as" -f "$known" >/dev/null; then
      echo "deploy.sh: $address already has a pinned key in $known: remove it first (ssh-keygen -R) if it changed" >&2; exit 1
    fi
    printf '%s\n' "$match" >> "$known"
    echo "deploy.sh: pinned $address's key ($want) in $known"
    ;;
  provision) [[ ${1:-} =~ $DOMAIN_RE ]] || { echo "deploy.sh: provision <domain>: lowercase and dotted" >&2; exit 2; }; remote provision "$1";;
  rollback|health|status) remote "$cmd";;
  accounts-add|accounts-revoke) [[ ${1:-} =~ $LABEL_RE ]] || { echo "deploy.sh: $cmd <label>: lowercase letters, digits and -" >&2; exit 2; }; remote "$cmd" "$1";;
  install-cert)
    cert=${1:?install-cert <cert.pem> <key.pem>} key=${2:?install-cert <cert.pem> <key.pem>}
    updir
    put_up "$cert" "$UPLOAD/cert.pem"; put_up "$key" "$UPLOAD/key.pem"
    remote install-cert "$UPLOAD/cert.pem" "$UPLOAD/key.pem"
    UPLOAD=""   # (the remote side removed it)
    ;;
  release)
    arch=$(ssh "${ssh_opts[@]}" -p "$port" "$host" uname -m < /dev/null)
    case "$arch" in x86_64) target=bun-linux-x64;; aarch64|arm64) target=bun-linux-arm64;; *) echo "deploy.sh: no Bun target for $arch" >&2; exit 1;; esac
    out=$(mktemp -d "${TMPDIR:-/tmp}/tamber-relay-build.XXXXXX")
    # One file, Bun inside it: the host needs nothing installed. Built from this checkout as it is.
    (cd "$root" && bun build --compile --target="$target" scripts/tamber-relay-server.mjs --outfile "$out/tamber-relay" >/dev/null)
    sha=$(shasum -a 256 "$out/tamber-relay" | cut -d' ' -f1)
    rev=$(git -C "$root" rev-parse --short=10 HEAD)$(git -C "$root" diff --quiet HEAD -- scripts || echo -dirty)
    echo "deploy.sh: tamber-relay $sha (from $rev, $target)"
    updir
    put_up "$out/tamber-relay" "$UPLOAD/tamber-relay"
    remote release "$sha" "$UPLOAD/tamber-relay"
    UPLOAD=""
    ;;
  check)
    domain=${1:?check <domain>}
    ok=1
    # From outside the host: what the internet sees.
    probe "$domain"
    san=$(printf '%s' "$PROBE" | sans)
    [ $HELD = 0 ] || { echo "check: FAILED: $domain's handshake was held open 10 s"; ok=0; }
    if [ "$san" = "DNS:$domain" ]; then echo "check: ${RELAY_CHECK_PORT:-443} answers $domain with its certificate"; else echo "check: FAILED: ${RELAY_CHECK_PORT:-443} did not answer $domain (got '${san:-nothing}')"; ok=0; fi
    probe unknown.invalid
    if [ $HELD = 1 ]; then echo "check: FAILED: an unknown name was held open 10 s, not closed"; ok=0
    else case "$PROBE" in *'BEGIN CERTIFICATE'*) echo "check: FAILED: an unknown name got a certificate"; ok=0;; *) echo "check: an unknown name is closed";; esac; fi
    if nc -z -w 3 "$address" "$port" 2>/dev/null; then echo "check: ssh ($port) answers"; else echo "check: FAILED: ssh ($port) does not answer"; ok=0; fi
    closed=()
    for p in 21 25 53 80 3000 5432 6379 7740 8080 8443 9090; do
      if nc -z -w 3 "$address" "$p" 2>/dev/null; then echo "check: FAILED: port $p is open"; ok=0; else closed+=("$p"); fi
    done
    echo "check: closed: ${closed[*]} (TCP over IPv4, common ports only; a full scan: nmap -Pn -p- $address, and nmap -6 for its IPv6 address)"
    [ $ok = 1 ]
    ;;
  *) echo "deploy.sh: unknown command $cmd" >&2; exit 2;;
esac
