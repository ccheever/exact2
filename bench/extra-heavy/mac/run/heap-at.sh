#!/bin/bash
# heap-at.sh <app> <label> [seconds-after-fling-start] — one fling run of <app> on this Mac (from ~/xhm), with
# `heap -s` (object counts and bytes by class and zone, no stack logging) taken from the live process at the
# fling's late, heavy phase (default 20 s after the probe starts scrolling, the 12k-24k segments), into
# tmp/heap-<label>.txt; the probe's JSON lands in tmp/hp-<label>.json. What the +230 MB default malloc zone is
# (PROGRAM.md loss 12: exact2 1491 vs SwiftUI 1044 MB at the fling's peak on bones, 2026-09-30).
X=${BENCH_STAGE:-$HOME/xhm}; cd $X; APP=$1; L=$2; AT=${3:-20}
rm -f tmp/running.pid tmp/heap-$L.txt
(BENCH_DELAY=15 RUN_LIMIT=150 ./run.sh $APP fling 0 $X/tmp/hp-$L.json > tmp/hp-$L.out 2>&1 &)
for i in $(seq 1 50); do [ -s tmp/running.pid ] && break; sleep 0.2; done
P=$(cat tmp/running.pid)
# The probe scrolls BENCH_DELAY after the window; the heap snapshot lands AT seconds into the scroll.
sleep $((15 + AT))
{ echo "heap of $APP pid $P at fling+$AT s, $(date +%T)"; /usr/bin/heap -s "$P" 2>&1; } > tmp/heap-$L.txt
for i in $(seq 1 90); do grep -q "ok\|timeout\|partial" tmp/hp-$L.out && break; sleep 1; done
cat tmp/hp-$L.out; wc -l tmp/heap-$L.txt
