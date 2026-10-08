#!/bin/bash
# series.sh — paired, alternating series on a simulator (devseries.sh's passes through run.sh).
# SERIES=<name> (results/<name>/), APPS="expo exact swiftui", ROUNDS=3; BENCH_LOCK as devseries.sh.
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../.." && pwd)
R=${BENCH_WORK:-$ROOT/target/bench/list}/results/${SERIES:-sim}; mkdir -p "$R"
for round in $(seq 1 ${ROUNDS:-3}); do
  if [ -n "$BENCH_LOCK" ]; then $BENCH_LOCK take listbench || exit 1; fi
  for app in ${APPS:-expo exact swiftui}; do
    ID=dev.exact.listbench.$app
    "$B/run.sh" $ID fling 0 "$R/$app-fling-t-$round.json"
    BENCH_RENDER=layer "$B/run.sh" $ID fling 4 "$R/$app-fling-b-$round.json"
    BENCH_RENDER=layer "$B/run.sh" $ID jump 1 "$R/$app-jump-layer-$round.json"
    "$B/run.sh" $ID jump 1 "$R/$app-jump-hier-$round.json"
  done
  if [ -n "$BENCH_LOCK" ]; then $BENCH_LOCK give listbench; fi
done
echo "series done: python3 $B/agg.py $R"
