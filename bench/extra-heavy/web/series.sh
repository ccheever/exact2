#!/bin/bash
# series.sh — the web pair's interleaved series in Chrome, run on the bench Mac from this directory (or a copy of
# it with dist/: `rsync -a web/ <host>:xhw/`). SERIES=<name> (results in $BENCH_RESULTS/<name>, default results/
# beside this script), APPS="exact2 expo", ROUNDS=3.
# Starts server.py (recorded PID, killed at the end), then per round and app: fling-t, ladder-t, rest,
# jump-layer (sampled every frame), coldstart, fling-b (blank sampling every 4th frame). File names are the
# native series', so `../summarize.py <results>/<series> exact2,expo` reads them.
W=$(cd "$(dirname "$0")" && pwd); export RESULTS=${BENCH_RESULTS:-$W/results}/${SERIES:-web-r1}; mkdir -p $RESULTS
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
CHROME_PIDFILE=$W/chrome.pid $PY $W/server.py $W $RESULTS > $RESULTS/server.log 2>&1 &
SERVER=$!; echo $SERVER > $W/server.pid; sleep 1
log() { echo "$(date +%T) $*" | tee -a $RESULTS/series.log; }
export BENCH_DELAY=${BENCH_DELAY:-8}
for round in $(seq 1 ${ROUNDS:-3}); do
  for app in ${APPS:-exact2 expo}; do
    log "round $round $app"
    $W/web.sh $app fling 0 $app-fling-t-$round.json | tee -a $RESULTS/series.log
    $W/web.sh $app ladder 0 $app-ladder-t-$round.json | tee -a $RESULTS/series.log
    $W/web.sh $app rest 0 $app-rest-$round.json | tee -a $RESULTS/series.log
    $W/web.sh $app jump 1 $app-jump-layer-$round.json | tee -a $RESULTS/series.log
    $W/web.sh $app coldstart 1 $app-coldstart-$round.json | tee -a $RESULTS/series.log
    $W/web.sh $app fling 4 $app-fling-b-$round.json | tee -a $RESULTS/series.log
    sleep 5
  done
done
kill $SERVER; rm -f $W/server.pid
log "series done"
