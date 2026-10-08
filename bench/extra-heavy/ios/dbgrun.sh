#!/bin/bash
# dbgrun.sh <bundle-id> <scenario> <sample> <out.json> <console.log> — one probe run on the device BENCH_DEVICE
# with the app's stdout/stderr (NSLog) captured through `devicectl … --console`; env as devrun.sh plus
# BENCH_METALDBG, BENCH_JUMPDUMP. Caller holds the device.
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}; BID=$1; S=$2; SMP=$3; OUT=$4; LOG=$5
T=$(mktemp -d)
timeout 60 xcrun devicectl device info apps --device $U --bundle-id $BID --json-output $T/app.json >/dev/null 2>&1
URL=$(python3 -c "import json;print(json.load(open('$T/app.json'))['result']['apps'][0]['url'].replace('file://',''))")
NAME="r-$(date +%s).json"
ENV="{\"DYLD_INSERT_LIBRARIES\":\"${URL}Frameworks/probe.dylib\",\"BENCH_SCENARIO\":\"$S\",\"BENCH_SAMPLE\":\"$SMP\",\"BENCH_OUT\":\"$NAME\",\"BENCH_DUMP\":\"${BENCH_DUMP:-}\",\"BENCH_RENDER\":\"${BENCH_RENDER:-}\",\"BENCH_DELAY\":\"${BENCH_DELAY:-15}\",\"BENCH_KINDS\":\"${BENCH_KINDS:-}\",\"BENCH_METALDBG\":\"${BENCH_METALDBG:-}\",\"BENCH_JUMPDUMP\":\"${BENCH_JUMPDUMP:-}\",\"BENCH_JUMPDUMP_ALL\":\"${BENCH_JUMPDUMP_ALL:-}\"}"
timeout ${DBG_SECS:-120} xcrun devicectl device process launch --console --device $U --terminate-existing --environment-variables "$ENV" $BID > "$LOG" 2>&1 &
CPID=$!
rm -f "$OUT"
for i in $(seq 1 30); do
  sleep 4
  timeout 60 xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME" --destination "$OUT" >/dev/null 2>&1 && [ -s "$OUT" ] && break
done
kill $CPID 2>/dev/null; wait $CPID 2>/dev/null
for D in $BENCH_DUMP $BENCH_JUMPDUMP; do
  mkdir -p "${OUT%.json}-$D"
  timeout 60 xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$D" --destination "${OUT%.json}-$D" >/dev/null 2>&1
done
rm -rf $T
[ -s "$OUT" ] && echo "ok $OUT" || echo "no result $OUT"
