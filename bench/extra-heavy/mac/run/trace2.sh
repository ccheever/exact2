#!/bin/bash
# trace2.sh <app> <label> [scenario] [BENCH_KINDS] — a Time Profiler of one probe run on this Mac (from ~/xhm),
# every thread, dumped compact to tmp/tp-<label>.json.gz (tmp/tpdump.py; analysis: tpx.py, anywhere). The run's
# probe JSON is tmp/tr-<label>.json (its segment times place the trace's window). The .trace is deleted.
X=${BENCH_STAGE:-$HOME/xhm}; cd $X; APP=$1; L=$2; SC=${3:-fling}; K=$4
XT=/Applications/Xcode.app/Contents/Developer/usr/bin/xctrace
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
rm -rf tmp/$L.trace tmp/running.pid tmp/tp-$L.json.gz tmp/tp-$L.xml
(BENCH_KINDS=$K BENCH_DELAY=${BENCH_DELAY:-15} RUN_LIMIT=150 ./run.sh $APP $SC 0 $X/tmp/tr-$L.json > tmp/tr-$L.out 2>&1 &)
for i in $(seq 1 50); do [ -s tmp/running.pid ] && break; sleep 0.2; done
P=$(cat tmp/running.pid); sleep ${LEAD:-8}
date +%s.%N > tmp/tr-$L.start 2>/dev/null
$XT record --template "Time Profiler" --attach $P --time-limit ${TL:-40s} --output tmp/$L.trace > tmp/tr-$L.xt 2>&1
for i in $(seq 1 60); do grep -q "ok\|timeout\|partial" tmp/tr-$L.out && break; sleep 1; done
$XT export --input tmp/$L.trace --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]' > tmp/tp-$L.xml 2>/dev/null
$PY tmp/tpdump.py tmp/tp-$L.xml tmp/tp-$L.json.gz
[ -n "$KEEPXML" ] || rm -f tmp/tp-$L.xml; [ -n "$KEEPTRACE" ] || rm -rf tmp/$L.trace
cat tmp/tr-$L.out
