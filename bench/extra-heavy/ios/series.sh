#!/bin/bash
# series.sh — the Extra Heavy interleaved series on one iOS device (the heavy-list probe's devrun.sh).
# SERIES=<name>, APPS="swiftui expo exact2 uikit", ROUNDS=3, BENCH_DEVICE=<udid>. Results: <app>-<scenario>-<round>.json in
# $BENCH_RESULTS/$SERIES (default <checkout>/target/bench/extra-heavy/results/$SERIES), with provenance.txt and crash/.
# Bundle ids: dev.exact.xheavy.<app> (expo = the LegendList build).
# Every lock take runs the probe's devclean.sh; every exit after a take gives the lock back; each devrun has a 7-minute
# wall-clock cap (RUN_CAP); two runs in a row that cannot launch (3 tries each) give the lock and stop the series
# (exit 3: the device stopped answering); a timed-out or partial run is retried once and a partial result is kept
# as <out>.partial (not summarized); crash logs are listed at the end of every hold.
# BENCH_LOCK=<command>: when set, `$BENCH_LOCK take <name>` / `$BENCH_LOCK give <name>` bracket each hold (a
# device shared with other runners); LOCK_NAME is the name (default xheavy). LOCK_PER_APP=1 takes it per app
# instead of per round (shorter holds; the round order is unchanged).
# INSTALL="<app.app> …": (re)installed at every take, before any run (another runner may install over the same
# bundle ids between takes); install.sh aborts the series on a failed install. INSTALL_MAP="<app>=<bundle.app> …"
# installs only the app about to run (LOCK_PER_APP=1); INSTALL= still installs all.
# DEVRUN=<script> overrides the probe's devrun.sh. FROM=<n> resumes at round n; SKIP_DONE=1 skips results on disk.
export LOCK_HOLDER_PID=$$  # for a BENCH_LOCK that reclaims the lock when its holder dies
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd)
P=${BENCH_PROBE:-$B/../heavy-list/probe}; D=${DEVRUN:-$P/devrun.sh}
RES=${BENCH_RESULTS:-$ROOT/target/bench/extra-heavy/results}; R=$RES/${SERIES:-series-r1}; mkdir -p $R
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID (xcrun devicectl list devices)}; export BENCH_DEVICE
SINCE=${CRASH_SINCE:-$(date +%Y-%m-%d-%H%M%S)}
HELD=; FAILS=0
N=${LOCK_NAME:-xheavy}
take() { [ -n "$BENCH_LOCK" ] && $BENCH_LOCK take $N; HELD=1; $P/devclean.sh; }
give() { [ -n "$HELD" ] && [ -n "$BENCH_LOCK" ] && $BENCH_LOCK give $N; HELD=; }
trap give EXIT; trap 'exit 1' INT TERM HUP
# drun <devrun args…>: devrun.sh, retried up to twice when devicectl fails to launch (an iOS 27 flake)
drun() {
  [ -n "$SKIP_DONE" ] && [ -s "${@: -1}" ] && return
  local out o="${@: -1}" again=1
  for t in 1 2 3; do
    out=$(timeout ${RUN_CAP:-420} "$D" "$@"); [ -z "$out" ] && out="launch failed"  # a devrun the cap killed prints nothing
    echo "$out"
    case "$out" in
      "launch failed") sleep 3; continue ;;
      ok*) FAILS=0; return ;;
      *) [ -s "$o" ] && mv "$o" "$o.partial"   # timeout / partial: the app hung or died; one more try
         [ $again = 1 ] || { FAILS=0; return; }; again=0; echo "$(date +%T) retrying once: $out"; sleep 3 ;;
    esac
  done
  [ "$out" = "launch failed" ] || { FAILS=0; return; }
  FAILS=$((FAILS+1))
  if [ $FAILS -ge 2 ]; then echo "$(date +%T) aborted: $U stopped launching (2 runs, 3 tries each); lock given back"; give; exit 3; fi
}
export BENCH_DELAY=${BENCH_DELAY:-15}
# BENCH_KINDS=17 leaves out the nested lists (filmstrip, inbox); unset (the default) is all 19 kinds plus the
# inner-list scenarios.
export BENCH_KINDS=${BENCH_KINDS-}
inst() { # [app]
  local what="$INSTALL"
  if [ -n "$1" ] && [ -n "$INSTALL_MAP" ]; then for kv in $INSTALL_MAP; do [ "${kv%%=*}" = "$1" ] && what="${kv#*=}"; done; fi
  [ -z "$what" ] && return
  $H/install.sh $U $what || { give; echo "aborted: install failed"; exit 1; }
}
# The app order rotates each round (an app run from a fixed position measured ~19 ms/s more CPU):
# round 1 as given, round 2 starting from the second app, round 3 from the third. ROTATE=0 keeps the given order.
rot() { local n=$1; shift; local a=("$@"); local k=$(( (n - 1) % ${#a[@]} )); [ "${ROTATE:-1}" = 0 ] && k=0; echo "${a[@]:$k} ${a[@]:0:$k}"; }
echo "$(date '+%F %T') series ${SERIES:-series-r1} on $U; checkout $(git -C "$ROOT" rev-parse --short=9 HEAD); probe sha1 $(shasum "$P/probe-ios.dylib" | cut -c1-12); apps ${APPS:-swiftui expo exact2 uikit}; BENCH_KINDS=${BENCH_KINDS:-all 19}" >> "$R/provenance.txt"
for round in $(seq ${FROM:-1} ${ROUNDS:-3}); do
  [ -z "$LOCK_PER_APP" ] && { take; inst; }
  for app in $(rot $round ${APPS:-swiftui expo exact2 uikit}); do
    # <name>@<series>: a second build of app <name> in the same rotation: its bundle is INSTALL_MAP's
    # <name>@<series> entry, its results go to $BENCH_RESULTS/<series>/<name>-….json
    full=$app; app=${full%%@*}; RR=$R; [ "$full" != "$app" ] && { RR=$RES/${full#*@}; mkdir -p $RR; }
    if [ -n "$SKIP_DONE" ] && [ -s $RR/$app-fling-t-$round.json ] && { [ -n "$BENCH_KINDS" ] || [ -s $RR/$app-innerkeep-$round.json ]; }; then continue; fi
    [ -n "$LOCK_PER_APP" ] && { take; inst $full; }
    ID=dev.exact.xheavy.$app
    echo "$(date +%T) round $round $full"
    drun $ID fling 0 $RR/$app-fling-t-$round.json
    BENCH_LIVE=1 drun $ID fling 0 $RR/$app-fling-live-$round.json
    drun $ID rest 0 $RR/$app-rest-$round.json
    drun $ID ladder 0 $RR/$app-ladder-t-$round.json
    BENCH_RENDER=layer drun $ID jump 1 $RR/$app-jump-layer-$round.json
    BENCH_RENDER=layer drun $ID coldstart 1 $RR/$app-coldstart-$round.json
    BENCH_RENDER=layer drun $ID fling 4 $RR/$app-fling-b-$round.json
    if [ -z "$BENCH_KINDS" ]; then  # the nested lists exist only in the 19-kind feed
      drun $ID innerfling 0 $RR/$app-innerfling-t-$round.json
      BENCH_RENDER=layer drun $ID innerfling 4 $RR/$app-innerfling-b-$round.json
      drun $ID innerkeep 0 $RR/$app-innerkeep-$round.json
    fi
    [ -n "$LOCK_PER_APP" ] && { $P/crashcheck.sh $SINCE $R/crash; give; }
  done
  [ -z "$LOCK_PER_APP" ] && { $P/crashcheck.sh $SINCE $R/crash; give; }
  sleep 20
done
echo series done
