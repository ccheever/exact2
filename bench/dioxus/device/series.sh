#!/bin/bash
# series.sh — the heavy list on a device: exact2's whole feed against the bounded window, bench/heavy-list's probe.
# Per round: devclean, install both (aborts on a failed install), then per app (the order alternates by round)
# fling-t, fling-b (blank-sampled) and coldstart (inserted probe); LADDER=1 runs ladder-t and ladder-b instead (the
# ladder crosses the bounded window's shifts; the fling never leaves the first window). Results:
# <checkout>/target/bench/dioxus/results/$SERIES/; summarize with
#   python3 bench/heavy-list/probe/summarize.py <dir> whole,bounded   (agg.py <dir> for the ladder's per-speed rows)
#   BENCH_DEVICE=<udid> required   SERIES=device-r1   ROUNDS=3 (LADDER: 2)   BENCH_LOCK=<command> optional
set -u
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd); P=$B/../heavy-list/probe; D=$P/devrun.sh
: "${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}"
R=${BENCH_RESULTS:-$ROOT/target/bench/dioxus/results}/${SERIES:-device-r1}; mkdir -p "$R"
export LOCK_HOLDER_PID=$$
WHOLE=$B/../heavy-list/exact-heavylist/build/HeavyExact2.app; BOUNDED=$H/build/HeavyBounded.app
echo "$(date '+%F %T') on $BENCH_DEVICE; whole $(cat "$WHOLE.commit" 2>/dev/null || echo ?) bounded $(cat "$BOUNDED.commit" 2>/dev/null || echo ?); probe sha1 $(shasum "$P/probe-ios.dylib" | cut -c1-12)" >> "$R/provenance.txt"
L() { [ -z "${BENCH_LOCK:-}" ] || $BENCH_LOCK "$1" dioxus; }
ROUNDS=${ROUNDS:-$([ -n "${LADDER:-}" ] && echo 2 || echo 3)}
for round in $(seq 1 $ROUNDS); do
  L take; "$P/devclean.sh"
  for a in "$WHOLE" "$BOUNDED"; do
    xcrun devicectl device install app --device $BENCH_DEVICE "$a" 2>&1 | grep -q "App installed" || { echo "INSTALL FAILED $a"; L give; exit 1; }
  done
  if [ $((round % 2)) = 1 ]; then order="whole bounded"; else order="bounded whole"; fi
  for app in $order; do
    [ $app = whole ] && ID=dev.exact.heavybench.exact || ID=dev.exact.heavybench.bounded
    echo "$(date +%T) round $round $app"
    if [ -n "${LADDER:-}" ]; then
      $D $ID ladder 0 $R/$app-ladder-t-$round.json
      BENCH_RENDER=layer $D $ID ladder 4 $R/$app-ladder-b-$round.json
    else
      $D $ID fling 0 $R/$app-fling-t-$round.json
      BENCH_RENDER=layer $D $ID fling 4 $R/$app-fling-b-$round.json
      BENCH_RENDER=layer $D $ID coldstart 1 $R/$app-coldstart-$round.json
    fi
  done
  L give
  sleep 20
done
echo series done
