#!/bin/sh
# A T3_SSH_COMMAND test double for T3Ssh.swift: never a real host, never ~/.ssh.
#   -G <alias>            prints `hostname <alias>`, `user tester`, `port 22`
#   host "unreachable"    fails as a refused connection
#   FAKE_SSH_PASSWORD     the host wants this password: BatchMode=yes is refused with ssh's
#                         "Permission denied (publickey,password,keyboard-interactive).", other
#                         runs ask $SSH_ASKPASS (as ssh does with SSH_ASKPASS_REQUIRE=force) and
#                         compare (FAKE_SSH_PASSWORD_FILE instead holds a password a test can
#                         change); FAKE_SSH_AUTH_LOG gets one line per attempt, never the secret
#   -N -L l:127.0.0.1:r   forwards loopback port l to r with node (FAKE_SSH_NODE)
#   FAKE_SSH_PASSWORD_HOSTS  limits the password to these host names (others take the "key")
#   <command…>            runs locally with HOME=$FAKE_SSH_REMOTE_HOME (or $FAKE_SSH_REMOTE_HOMES/<host>
#                         when that folder exists) and FAKE_SSH_REMOTE_PATH first on PATH (a `t3` shim)
batch=no; forward=""; resolve=""; host=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) [ "$2" = "BatchMode=yes" ] && batch=yes; shift 2 ;;
    -F|-p) shift 2 ;;
    -L) forward=$2; shift 2 ;;
    -G) resolve=$2; shift 2 ;;
    -*) shift ;;
    *) host=$1; shift; break ;;
  esac
done
if [ -n "$resolve" ]; then printf 'hostname %s\nuser tester\nport 22\n' "$resolve"; exit 0; fi
name=${host#*@}
if [ "$name" = "unreachable" ]; then echo "ssh: connect to host unreachable port 22: Connection refused" >&2; exit 255; fi
[ -n "$FAKE_SSH_PASSWORD_FILE" ] && FAKE_SSH_PASSWORD=$(cat "$FAKE_SSH_PASSWORD_FILE")
log() { [ -n "$FAKE_SSH_AUTH_LOG" ] && printf '%s\n' "$1" >>"$FAKE_SSH_AUTH_LOG"; return 0; }
needs_password=yes
if [ -n "$FAKE_SSH_PASSWORD_HOSTS" ]; then case " $FAKE_SSH_PASSWORD_HOSTS " in *" $name "*) ;; *) needs_password=no ;; esac; fi
if [ -n "${FAKE_SSH_PASSWORD+x}" ] && [ "$needs_password" = yes ]; then
  if [ "$batch" = yes ]; then log "batch refused $name"; echo "$host: Permission denied (publickey,password,keyboard-interactive)." >&2; exit 255; fi
  given=""
  if [ -n "$SSH_ASKPASS" ] && [ "$SSH_ASKPASS_REQUIRE" = force ]; then given=$("$SSH_ASKPASS" "$host's password: " 2>/dev/null) || given=""; fi
  if [ "$given" != "$FAKE_SSH_PASSWORD" ]; then log "password refused $name"; echo "$host: Permission denied (publickey,password,keyboard-interactive)." >&2; exit 255; fi
  log "password accepted $name"
fi
if [ -n "$forward" ]; then
  local_port=${forward%%:*}; remote_port=${forward##*:}
  exec "${FAKE_SSH_NODE:-node}" -e '
    const net = require("net"), [l, r] = process.argv.slice(1).map(Number);
    net.createServer(c => { const u = net.connect(r, "127.0.0.1"); c.pipe(u).pipe(c); u.on("error", () => c.destroy()); c.on("error", () => u.destroy()); }).listen(l, "127.0.0.1");
  ' "$local_port" "$remote_port"
fi
[ $# -eq 0 ] && exit 0
remote_home=${FAKE_SSH_REMOTE_HOME:-$HOME}
[ -n "$FAKE_SSH_REMOTE_HOMES" ] && [ -d "$FAKE_SSH_REMOTE_HOMES/$name" ] && remote_home="$FAKE_SSH_REMOTE_HOMES/$name"
HOME=$remote_home PATH="${FAKE_SSH_REMOTE_PATH:+$FAKE_SSH_REMOTE_PATH:}$PATH" exec "$@"
