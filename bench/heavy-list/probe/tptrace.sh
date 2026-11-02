#!/bin/bash
# tptrace.sh <bundle-id> <scenario> <outdir> [KEY=VAL …] — Time Profiler around one probe run on the device
# (BENCH_DEVICE); the caller holds the device lock, if any. Records <outdir>/run.trace
# and copies the probe JSON to <outdir>/probe.json. BENCH_DELAY defaults to 15.
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}; BID=$1; S=$2; O=$3; shift 3; mkdir -p $O
J=$O/.app.json
xcrun devicectl device info apps --device $U --bundle-id $BID --json-output $J >/dev/null 2>&1
URL=$(python3 -c "import json;print(json.load(open('$J'))['result']['apps'][0]['url'].replace('file://',''))")
NAME="t-$(date +%s).json"; EXTRA=()
for kv in "$@"; do EXTRA+=(--env "$kv"); done
rm -rf $O/run.trace
xcrun xctrace record --template 'Time Profiler' --device $U --time-limit ${TLIMIT:-60}s --output $O/run.trace \
  --env DYLD_INSERT_LIBRARIES=${URL}Frameworks/probe.dylib --env BENCH_SCENARIO=$S --env BENCH_SAMPLE=0 --env BENCH_OUT=$NAME \
  --env BENCH_DELAY=${BENCH_DELAY:-15} "${EXTRA[@]}" --launch -- $BID > $O/xctrace.log 2>&1
xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME" --destination $O/probe.json >/dev/null 2>&1
# Export after the lock is given back, then read it with tpagg.py / tpcallee.py / tpcaller.py:
#   xcrun xctrace export --input $O/run.trace --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]' > $O/tp.xml
ls -la $O/probe.json
