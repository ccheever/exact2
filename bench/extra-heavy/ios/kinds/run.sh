#!/bin/bash
export LOCK_HOLDER_PID=$$  # for a BENCH_LOCK that reclaims the lock when its holder dies
# run.sh <round> <kind[,kind…]|17>… — the per-kind breakdown on BENCH_DEVICE: per kind, one hold, fling 0 + ladder 0
# on each app with BENCH_KINDS=<kind> (the app order rotates per kind and round). APPS (default "swiftui exact2 expo").
# Out: $BENCH_RESULTS/$SERIES/r<round>/<tag>/<app>-<scen>-1.json (default <checkout>/target/bench/extra-heavy/results/
# kinds); agg.py reads it (KROOT=<that directory>), table.py prints agg.py's JSON.
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/../.." && pwd)
P=${BENCH_PROBE:-$B/../heavy-list/probe}
R=${BENCH_RESULTS:-$B/../../target/bench/extra-heavy/results}/${SERIES:-kinds}; round=$1; shift
: "${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}"
# drun <devrun args…>: devrun.sh, retried up to twice when devicectl fails to launch (an iOS 27 flake)
drun() { for t in 1 2 3; do out=$("$P/devrun.sh" "$@"); echo "$out"; [ "$out" = "launch failed" ] || return; sleep 3; done; }
export BENCH_DELAY=15
n=0
for K in "$@"; do
  tag=${K//,/+}; O=$R/r$round/$tag; mkdir -p $O
  set -- ${APPS:-swiftui exact2 expo}; apps=("$@")
  k=$(( (n + round) % ${#apps[@]} )); ord=("${apps[@]:$k}" "${apps[@]:0:$k}"); n=$((n+1))
  [ -n "$BENCH_LOCK" ] && $BENCH_LOCK take kinds >/dev/null
  for app in "${ord[@]}"; do
    echo "$(date +%T) r$round $tag $app"
    BENCH_KINDS=$K drun dev.exact.xheavy.$app fling 0 $O/$app-fling-t-1.json
    BENCH_KINDS=$K drun dev.exact.xheavy.$app ladder 0 $O/$app-ladder-t-1.json
  done
  [ -n "$BENCH_LOCK" ] && $BENCH_LOCK give kinds >/dev/null
  sleep 15
done
echo "round $round done"
