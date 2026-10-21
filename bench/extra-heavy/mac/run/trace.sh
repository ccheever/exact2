#!/bin/bash
# trace.sh <app> <label> [BENCH_KINDS] — a 40 s Time Profiler of a fling run on this Mac (from ~/xhm), aggregated
# to tmp/tp-<label>.txt and tmp/callee-<label>.txt (tpagg.py / tpcallee.py / tpcaller.py in tmp/); the trace is deleted.
X=${BENCH_STAGE:-$HOME/xhm}; cd $X; APP=$1; L=$2; K=$3; SC=${4:-fling}
XT=/Applications/Xcode.app/Contents/Developer/usr/bin/xctrace
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
rm -rf tmp/$L.trace tmp/running.pid
(BENCH_KINDS=$K BENCH_DELAY=15 RUN_LIMIT=150 ./run.sh $APP $SC 0 $X/tmp/tr-$L.json > tmp/tr-$L.out 2>&1 &)
for i in $(seq 1 50); do [ -s tmp/running.pid ] && break; sleep 0.2; done
P=$(cat tmp/running.pid); sleep 8
$XT record --template "Time Profiler" --attach $P --time-limit ${TL:-40s} --output tmp/$L.trace > /dev/null 2>&1
for i in $(seq 1 60); do grep -q "ok\|timeout\|partial" tmp/tr-$L.out && break; sleep 1; done
$XT export --input tmp/$L.trace --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]' > /tmp/tp.xml 2>/dev/null
cd tmp && $PY tpagg.py > tp-$L.txt && $PY tpcallee.py "Presenter.apply" "CA::Context::commit_transaction" "NSViewBackingLayer display]" "Frames.tick" "wantsUpdateLayer.getter" "NodeView.updateLayer" "NodeView.draw(_:)" > callee-$L.txt
$PY tpcaller.py NSAccessibilitySetObjectValueForAttribute "postNotificationName:object:userInfo:" "NodeView.boxPlan" "_updateTrackingAreasWithInvalidCursorRects" "-[NSViewBackingLayer drawInContext:]" > caller-$L.txt 2>/dev/null
rm -f /tmp/tp.xml; rm -rf $L.trace
