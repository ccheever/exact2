#!/bin/bash
# devseries.sh — paired, alternating series on the phone (BENCH_DEVICE): per round and app, a
# timing fling, a blank-sampling fling, and jumps sampled by layer and by view hierarchy.
# SERIES=<name> (results/<name>/), APPS="expo exact swiftui", ROUNDS=3. BENCH_LOCK, when set,
# is a command run as `$BENCH_LOCK take listbench` before each round and `give listbench` after.
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../.." && pwd)
R=${BENCH_WORK:-$ROOT/target/bench/list}/results/${SERIES:-dev}; mkdir -p "$R"
for round in $(seq 1 ${ROUNDS:-3}); do
  if [ -n "$BENCH_LOCK" ]; then $BENCH_LOCK take listbench || exit 1; fi
  for app in ${APPS:-expo exact swiftui}; do
    ID=dev.exact.listbench.$app
    "$B/devrun.sh" $ID fling 0 "$R/$app-fling-t-$round.json"
    BENCH_RENDER=layer "$B/devrun.sh" $ID fling 4 "$R/$app-fling-b-$round.json"
    BENCH_RENDER=layer "$B/devrun.sh" $ID jump 1 "$R/$app-jump-layer-$round.json"
    "$B/devrun.sh" $ID jump 1 "$R/$app-jump-hier-$round.json"
  done
  if [ -n "$BENCH_LOCK" ]; then $BENCH_LOCK give listbench; fi
done
echo "devseries done: python3 $B/agg.py $R"
