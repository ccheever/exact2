#!/usr/bin/env bash
# Deploys the tamber relay (scripts/tamber-relay-server.mjs, docs/tamber-relay-service.md) to one Ubuntu or
# Debian host over SSH, from a checkout on the developer's machine. Idempotent; the runbook is
# ~/x2theme/r4-deploy-runbook.md (LLP 1121 §5 10c R4).
#
#   relay/deploy/deploy.sh <host> provision <domain>   the host made ready (relay/deploy/remote.sh provision)
#   relay/deploy/deploy.sh <host> release              build the relay for the host's architecture, upload, switch, health-check
#   relay/deploy/deploy.sh <host> rollback|health|status
#   relay/deploy/deploy.sh <host> install-cert <cert.pem> <key.pem>   the relay's own certificate (the ACME step's)
#   relay/deploy/deploy.sh <host> accounts-add <label>      its token printed once, here: never paste it into chat
#   relay/deploy/deploy.sh <host> accounts-revoke <label>
#   relay/deploy/deploy.sh <host> check <domain>       from here: 443 answers <domain>, an unknown name is closed,
#                                                     and nothing else is open (22 and 443 only)
#
# <host> is [user@]address. Environment: RELAY_SSH_KEY (default ~/.ssh/tamber_relay_vps), RELAY_SSH_PORT (22),
# RELAY_KNOWN_HOSTS (default ~/.ssh/known_hosts; the first connection records the host's key), RELAY_CHECK_PORT (443,
# where check reaches the relay: a test VM's forwarded port). A user other than
# root runs the remote side through sudo.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
host=${1:?usage: deploy.sh <[user@]host> <provision <domain>|release|rollback|health|status|install-cert <cert> <key>|accounts-add <label>|accounts-revoke <label>|check <domain>>}
cmd=${2:?a command}
shift 2
key_file=${RELAY_SSH_KEY:-$HOME/.ssh/tamber_relay_vps}
port=${RELAY_SSH_PORT:-22}
ssh_opts=(-i "$key_file" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=15 -p "$port")
[ -n "${RELAY_KNOWN_HOSTS:-}" ] && ssh_opts+=(-o UserKnownHostsFile="$RELAY_KNOWN_HOSTS")
user=root; [[ $host == *@* ]] && user=${host%%@*}
sudo=""; [ "$user" = root ] || sudo="sudo"
address=${host#*@}

remote() { ssh "${ssh_opts[@]}" "$host" "$sudo bash -s -- $*" < "$here/remote.sh"; }

case "$cmd" in
  provision) remote provision "${1:?provision <domain>}";;
  rollback|health|status) remote "$cmd";;
  accounts-add|accounts-revoke) remote "$cmd" "${1:?$cmd <label>}";;
  install-cert)
    cert=${1:?install-cert <cert.pem> <key.pem>} key=${2:?install-cert <cert.pem> <key.pem>}
    stamp=$(date +%s)
    scp -q -i "$key_file" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -P "$port" ${RELAY_KNOWN_HOSTS:+-o UserKnownHostsFile="$RELAY_KNOWN_HOSTS"} "$cert" "$host:/tmp/relay-cert.$stamp"
    scp -q -i "$key_file" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -P "$port" ${RELAY_KNOWN_HOSTS:+-o UserKnownHostsFile="$RELAY_KNOWN_HOSTS"} "$key" "$host:/tmp/relay-key.$stamp"
    remote install-cert "/tmp/relay-cert.$stamp" "/tmp/relay-key.$stamp"
    ;;
  release)
    arch=$(ssh "${ssh_opts[@]}" "$host" uname -m < /dev/null)
    case "$arch" in x86_64) target=bun-linux-x64;; aarch64|arm64) target=bun-linux-arm64;; *) echo "deploy.sh: no Bun target for $arch" >&2; exit 1;; esac
    out=$(mktemp -d "${TMPDIR:-/tmp}/tamber-relay-build.XXXXXX")
    trap 'rm -rf "$out"' EXIT
    # One file, Bun inside it: the host needs nothing installed. Built from this checkout as it is.
    (cd "$root" && bun build --compile --target="$target" scripts/tamber-relay-server.mjs --outfile "$out/tamber-relay" >/dev/null)
    sha=$(shasum -a 256 "$out/tamber-relay" | cut -d' ' -f1)
    rev=$(git -C "$root" rev-parse --short=10 HEAD)$(git -C "$root" diff --quiet HEAD -- scripts || echo -dirty)
    echo "deploy.sh: tamber-relay $sha (from $rev, $target)"
    upload=/tmp/tamber-relay.$sha
    scp -q -i "$key_file" -o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new -P "$port" ${RELAY_KNOWN_HOSTS:+-o UserKnownHostsFile="$RELAY_KNOWN_HOSTS"} "$out/tamber-relay" "$host:$upload"
    remote release "$sha" "$upload"
    ;;
  check)
    domain=${1:?check <domain>}
    ok=1
    # From outside the host: what the internet sees.
    san=$(openssl s_client -connect "$address:${RELAY_CHECK_PORT:-443}" -servername "$domain" </dev/null 2>/dev/null | openssl x509 -noout -ext subjectAltName 2>/dev/null | tail -1 | tr -d ' ' || true)
    if [ "$san" = "DNS:$domain" ]; then echo "check: 443 answers $domain with its certificate"; else echo "check: FAILED: 443 did not answer $domain (got '${san:-nothing}')"; ok=0; fi
    if openssl s_client -connect "$address:${RELAY_CHECK_PORT:-443}" -servername "unknown.invalid" </dev/null 2>/dev/null | grep -q 'BEGIN CERTIFICATE'; then echo "check: FAILED: an unknown name got a certificate"; ok=0; else echo "check: an unknown name is closed"; fi
    for p in 80 8080 3000 7740 8443; do
      if nc -z -w 3 "$address" "$p" 2>/dev/null; then echo "check: FAILED: port $p is open"; ok=0; fi
    done
    [ $ok = 1 ] && echo "check: only 22 and 443 answer (80, 8080, 3000, 7740, 8443 closed)"
    [ $ok = 1 ]
    ;;
  *) echo "deploy.sh: unknown command $cmd" >&2; exit 2;;
esac
