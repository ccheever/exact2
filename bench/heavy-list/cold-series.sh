#!/bin/bash
# cold-series.sh — cold start without DYLD_INSERT_LIBRARIES (which disables an app's prebuilt launch closure): the
# probe linked into a copy of each app (probe/makecold.sh, bundle ids dev.exact.heavybench.<app>cold), launched by
# probe/devrun-cold.sh, interleaved rounds, one lock hold per round. Installs every copy in BENCH_COLD first and
# aborts on a failed install. Results are <app>-coldstart-<round>.json in
# $BENCH_RESULTS/$SERIES (default <checkout>/target/bench/heavy-list/results/cold); coldsum.py summarizes them.
#   BENCH_DEVICE=<udid>  required     SERIES=cold     APPS="swiftui exact expo uikit"     ROUNDS=3
#   BENCH_COLD=<dir>     the cold copies (default <checkout>/target/bench/heavy-list/cold)
#   BENCH_LOCK=<command> optional: `<command> take cold` / `<command> give cold` around each hold
#   BENCH_ID_PREFIX      the bundle ids' prefix (default dev.exact.heavybench.; bench/crypto-list sets its own)
set -u
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd); D=$H/probe/devrun-cold.sh
export BENCH_DEVICE=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID} LOCK_HOLDER_PID=$$ BENCH_RENDER=layer
C=${BENCH_COLD:-$ROOT/target/bench/heavy-list/cold}
R=${BENCH_RESULTS:-$ROOT/target/bench/heavy-list/results}/${SERIES:-cold}; mkdir -p "$R"
L() { [ -z "${BENCH_LOCK:-}" ] || $BENCH_LOCK "$1" cold; }
L take
for a in "$C"/*.app; do
  xcrun devicectl device install app --device $BENCH_DEVICE "$a" 2>&1 | grep -q "App installed" && echo "installed $a" || { echo "INSTALL FAILED $a"; L give; exit 1; }
done
L give
[ -s "$C/PROVENANCE.txt" ] && cp "$C/PROVENANCE.txt" "$R/"
for r in $(seq 1 ${ROUNDS:-3}); do
  L take
  for app in ${APPS:-swiftui exact expo uikit}; do
    for t in 1 2 3; do o=$($D ${BENCH_ID_PREFIX:-dev.exact.heavybench.}${app}cold coldstart 1 $R/$app-coldstart-$r.json); echo "$o"; [ "$o" = "launch failed" ] || break; sleep 3; done
  done
  L give
done
echo cold done
