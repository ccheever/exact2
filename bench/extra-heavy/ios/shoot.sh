#!/bin/bash
# shoot.sh <bundle-id> <name> [start-index…] — BENCH_FREEZE=1 screenshots on the device BENCH_DEVICE into
# $BENCH_SHOTS/<name>-freeze-<top|n>.png (default <checkout>/target/bench/extra-heavy/shots): the parity check
# between the apps. Start indices 3 = filmstrip, 7 = inbox. Hold the device (BENCH_LOCK) around it. Waits 12 s per
# shot (map tiles).
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}; BID=$1; NAME=$2; shift 2
H=$(cd "$(dirname "$0")" && pwd); O=${BENCH_SHOTS:-$H/../../../target/bench/extra-heavy/shots}; mkdir -p "$O"
for i in top "$@"; do
  E='{"BENCH_FREEZE":"1"}'; [ "$i" != top ] && E="{\"BENCH_FREEZE\":\"1\",\"BENCH_START_INDEX\":\"$i\"}"
  xcrun devicectl device process launch --device $U --terminate-existing --environment-variables "$E" $BID >/dev/null 2>&1 || { echo "launch failed"; continue; }
  sleep 12
  xcrun devicectl device capture screenshot --device $U --destination "$O/$NAME-freeze-$i.png" >/dev/null 2>&1 && echo "$O/$NAME-freeze-$i.png" || echo "capture failed $i"
done
