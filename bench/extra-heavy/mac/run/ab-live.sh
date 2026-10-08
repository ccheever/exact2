#!/bin/bash
# ab-live.sh — ROUNDS (6) interleaved live-only fling-t runs of APPS on this Mac, into results/$SERIES/live.
X=${BENCH_STAGE:-$HOME/xhm}; R=$X/results/${SERIES:-ab-live}; mkdir -p $R/live
for round in $(seq 1 ${ROUNDS:-6}); do for app in ${APPS:-exact2main exact2next}; do
  BENCH_KINDS=live BENCH_DELAY=15 $X/run.sh $app fling 0 $R/live/$app-fling-t-$round.json | tee -a $R/series.log
done; done; echo "series done" >> $R/series.log
