#!/bin/bash
# run.sh <bundle-id> <scenario> <sample 0|1> <out.json> — one probe run on a simulator
# (BENCH_SIM, default the booted one). BENCH_DYLIB=diag runs the layer diagnostic instead.
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../.." && pwd)
W=${BENCH_WORK:-$ROOT/target/bench/list}
U=${BENCH_SIM:-booted}
BID=$1; S=$2; SMP=$3; OUT=$4
case "$OUT" in /*) ;; *) OUT=$PWD/$OUT ;; esac
rm -f "$OUT"
xcrun simctl terminate "$U" $BID >/dev/null 2>&1
sleep 1
SIMCTL_CHILD_DYLD_INSERT_LIBRARIES=$W/${BENCH_DYLIB:-probe}-sim.dylib \
SIMCTL_CHILD_BENCH_SCENARIO=$S SIMCTL_CHILD_BENCH_SAMPLE=$SMP SIMCTL_CHILD_BENCH_OUT=$OUT \
SIMCTL_CHILD_BENCH_DUMP=${BENCH_DUMP:-} SIMCTL_CHILD_BENCH_RENDER=${BENCH_RENDER:-} \
  xcrun simctl launch "$U" $BID >/dev/null
for i in $(seq 1 120); do [ -s "$OUT" ] && break; sleep 1; done
xcrun simctl terminate "$U" $BID >/dev/null 2>&1
[ -s "$OUT" ] && echo "ok $OUT" || echo "timeout $OUT"
