#!/bin/bash
# trace.sh <app: swiftui|exact2|expo|uikit> <kinds> <outdir> — Time Profiler on BENCH_DEVICE around one `fling 0` of a
# BENCH_KINDS=<kinds> feed (caller holds the device); tpk.py summarizes the fling window into <outdir>/tp.txt.
H=$(cd "$(dirname "$0")" && pwd)
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}; BID=dev.exact.xheavy.$1; K=$2; O=$3; mkdir -p $O
J=$O/.app.json
xcrun devicectl device info apps --device $U --bundle-id $BID --json-output $J >/dev/null 2>&1
URL=$(python3 -c "import json;print(json.load(open('$J'))['result']['apps'][0]['url'].replace('file://',''))")
NAME="t-$(date +%s).json"
rm -rf $O/run.trace
xcrun xctrace record --template 'Time Profiler' --device $U --time-limit ${TLIM:-50}s --output $O/run.trace \
  --env DYLD_INSERT_LIBRARIES=${URL}Frameworks/probe.dylib --env BENCH_SCENARIO=fling --env BENCH_SAMPLE=0 --env BENCH_OUT=$NAME \
  --env BENCH_DELAY=15 --env BENCH_KINDS=$K --launch -- $BID > $O/xctrace.log 2>&1
xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME" --destination $O/probe.json >/dev/null 2>&1
xcrun xctrace export --input $O/run.trace --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]' > $O/tp.xml 2>$O/export.log
python3 $H/tpk.py $O/tp.xml 16.5 28 > $O/tp.txt 2>&1
rm -rf $O/run.trace
ls -la $O | tail -3
