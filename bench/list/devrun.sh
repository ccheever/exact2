#!/bin/bash
# devrun.sh <bundle-id> <scenario> <sample> <local-out.json> — one probe run on the phone
# BENCH_DEVICE names (UDID or name), through devicectl with DYLD_INSERT_LIBRARIES. The app must
# carry the probe (resign.sh). The result is written to the app's Documents and copied back.
U=${BENCH_DEVICE:?set BENCH_DEVICE to the phone (xcrun devicectl list devices)}; BID=$1; S=$2; SMP=$3; OUT=$4
INFO=$(mktemp -t listbench-app); trap 'rm -f "$INFO"' EXIT
xcrun devicectl device info apps --device "$U" --bundle-id $BID --json-output "$INFO" >/dev/null 2>&1
URL=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['result']['apps'][0]['url'].replace('file://',''))" "$INFO") \
  || { echo "$BID is not installed on $U"; exit 1; }
NAME="r-$(date +%s).json"
ENV="{\"DYLD_INSERT_LIBRARIES\":\"${URL}Frameworks/probe.dylib\",\"BENCH_SCENARIO\":\"$S\",\"BENCH_SAMPLE\":\"$SMP\",\"BENCH_OUT\":\"$NAME\",\"BENCH_DUMP\":\"${BENCH_DUMP:-}\",\"BENCH_RENDER\":\"${BENCH_RENDER:-}\"}"
xcrun devicectl device process launch --device "$U" --terminate-existing --environment-variables "$ENV" $BID >/dev/null 2>&1 || { echo "launch failed"; exit 1; }
rm -f "$OUT"
for i in $(seq 1 30); do
  sleep 4
  xcrun devicectl device copy from --device "$U" --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME" --destination "$OUT" >/dev/null 2>&1 && [ -s "$OUT" ] && break
done
[ -s "$OUT" ] && echo "ok $OUT" || echo "timeout $OUT"
