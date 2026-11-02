#!/bin/bash
# series.sh — the crypto list's interleaved scroll series on one device, with bench/heavy-list's probe. Per round and
# app: fling timing (fling-t), fling in live mode (fling-live: ticks on), rest (no scroll, ticks on, 10 s), ladder
# timing (ladder-t), jump (layer sampler), coldstart (inserted probe) and fling blank-sampled (fling-b). Results are
# <app>-<kind>-<round>.json in $BENCH_RESULTS/$SERIES (default <checkout>/target/bench/crypto-list/results/$SERIES),
# with provenance.txt and crash/. The heavy list's series also runs a blank-sampled ladder; this one never did.
#   BENCH_DEVICE=<udid>   required          SERIES=<name>   required
#   APPS="swiftui uikit svgi"               (also expo, gpu)     ROUNDS=3     START_ROUND=1
#   ROTATE=1              the app order rotates each round (round 2 starts from the second app); 0 keeps it
#   BENCH_DELAY=15        seconds from launch to the first measured segment (the probe's)
#   BENCH_LOCK=<command>  optional: `<command> take crypto` before each app's runs, `<command> give crypto` after
#   RUN_CAP=420           seconds one probe run may take
# The apps must be installed first, signed with the probe (README.md); bundle ids dev.exact.cryptobench.<app>.
set -u
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd); P=$H/../heavy-list/probe; D=$P/devrun.sh
: "${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}"
R=${BENCH_RESULTS:-$ROOT/target/bench/crypto-list/results}/${SERIES:?set SERIES to a name}; mkdir -p "$R"
export LOCK_HOLDER_PID=$$ BENCH_DELAY=${BENCH_DELAY:-15}  # a lock command may reclaim the lock if this script dies holding it
APPS=${APPS:-swiftui uikit svgi}
SINCE=$(date +%Y-%m-%d-%H%M%S); HELD=
take() { [ -n "${BENCH_LOCK:-}" ] && $BENCH_LOCK take crypto; HELD=1; "$P/devclean.sh"; }
give() { [ -n "$HELD" ] && [ -n "${BENCH_LOCK:-}" ] && $BENCH_LOCK give crypto; HELD=; }
trap give EXIT; trap 'exit 1' INT TERM HUP
# One probe run: "launch failed" is tried three times; a timed-out or partial run once more (the partial result is
# kept as <out>.partial, which no summary reads).
run() {
  local out o="${@: -1}" again=1
  for t in 1 2 3; do
    out=$(timeout ${RUN_CAP:-420} "$D" "$@"); [ -z "$out" ] && out="launch failed"  # a run the cap killed prints nothing
    echo "$out"
    case "$out" in
      "launch failed") sleep 3 ;;
      ok*) return ;;
      *) [ -s "$o" ] && mv "$o" "$o.partial"; [ $again = 1 ] || return; again=0; sleep 3 ;;
    esac
  done
}
rot() { local n=$1; shift; local a=("$@"); local k=$(( (n - 1) % ${#a[@]} )); [ "${ROTATE:-1}" = 0 ] && k=0; echo "${a[@]:$k} ${a[@]:0:$k}"; }
echo "$(date '+%F %T') series $SERIES on $BENCH_DEVICE; checkout $(git -C "$ROOT" rev-parse --short=9 HEAD); probe sha1 $(shasum "$P/probe-ios.dylib" | cut -c1-12); apps $APPS" >> "$R/provenance.txt"
for round in $(seq ${START_ROUND:-1} ${ROUNDS:-3}); do
  for app in $(rot $round $APPS); do
    ID=dev.exact.cryptobench.$app
    take
    echo "$(date +%T) round $round $app"
    run $ID fling 0 $R/$app-fling-t-$round.json
    BENCH_LIVE=1 run $ID fling 0 $R/$app-fling-live-$round.json
    run $ID rest 0 $R/$app-rest-$round.json
    run $ID ladder 0 $R/$app-ladder-t-$round.json
    BENCH_RENDER=layer run $ID jump 1 $R/$app-jump-layer-$round.json
    BENCH_RENDER=layer run $ID coldstart 1 $R/$app-coldstart-$round.json
    BENCH_RENDER=layer run $ID fling 4 $R/$app-fling-b-$round.json
    "$P/crashcheck.sh" $SINCE "$R/crash"
    give
  done
  sleep 20
done
echo series done
