#!/bin/bash
# run.sh <bundle-id> <scenario> <sample> <out.json> — one run on a simulator (BENCH_SIM=<udid>)
# Passes BENCH_RENDER, BENCH_DUMP, BENCH_DELAY and BENCH_LIVE through when set.
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
B=$(cd "$(dirname "$0")" && pwd)
U=${BENCH_SIM:?set BENCH_SIM to a simulator udid}
BID=$1; S=$2; SMP=$3; OUT=$4
rm -f "$OUT"
xcrun simctl terminate $U $BID >/dev/null 2>&1
sleep 1
export SIMCTL_CHILD_DYLD_INSERT_LIBRARIES=$B/probe-sim.dylib
export SIMCTL_CHILD_BENCH_SCENARIO=$S SIMCTL_CHILD_BENCH_SAMPLE=$SMP SIMCTL_CHILD_BENCH_OUT=$OUT
for v in BENCH_RENDER BENCH_DUMP BENCH_DELAY BENCH_LIVE; do
  [ -n "${!v}" ] && export SIMCTL_CHILD_$v="${!v}"
done
xcrun simctl launch $U $BID >/dev/null
for i in $(seq 1 120); do [ -s "$OUT" ] && break; sleep 1; done
xcrun simctl terminate $U $BID >/dev/null 2>&1
[ -s "$OUT" ] && echo "ok $OUT" || echo "timeout $OUT"
