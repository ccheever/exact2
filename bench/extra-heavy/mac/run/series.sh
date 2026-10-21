#!/bin/bash
# series.sh — the xheavy macOS interleaved series, run ON the bench Mac from ~/xhm (bones or silver).
# SERIES=<name> (results/<name>/), APPS="swiftui exact2", ROUNDS=3, BENCH_KINDS unset = the 19-kind feed.
# PART=feed (the iOS series' scenarios) | kinds (per-kind breakdown: fling-t + ladder-t for each of KINDS).
X=${BENCH_STAGE:-$HOME/xhm}; R=$X/results/${SERIES:-mac-r1}; mkdir -p $R
export BENCH_DELAY=${BENCH_DELAY:-15}
KINDS=${KINDS:-shader video map webview carousel svg live canvas}
log() { echo "$(date +%T) $*" | tee -a $R/series.log; }
for round in $(seq 1 ${ROUNDS:-3}); do
  for app in ${APPS:-swiftui exact2}; do
    log "round $round $app ${PART:-feed}"
    if [ "${PART:-feed}" = feed ]; then
      $X/run.sh $app fling 0 $R/$app-fling-t-$round.json | tee -a $R/series.log
      BENCH_LIVE=1 $X/run.sh $app fling 0 $R/$app-fling-live-$round.json | tee -a $R/series.log
      $X/run.sh $app rest 0 $R/$app-rest-$round.json | tee -a $R/series.log
      $X/run.sh $app ladder 0 $R/$app-ladder-t-$round.json | tee -a $R/series.log
      BENCH_RENDER=window $X/run.sh $app jump 1 $R/$app-jump-layer-$round.json | tee -a $R/series.log
      BENCH_RENDER=window $X/run.sh $app coldstart 1 $R/$app-coldstart-$round.json | tee -a $R/series.log
      BENCH_RENDER=window $X/run.sh $app fling 4 $R/$app-fling-b-$round.json | tee -a $R/series.log
      if [ -z "$BENCH_KINDS" ]; then
        $X/run.sh $app innerfling 0 $R/$app-innerfling-t-$round.json | tee -a $R/series.log
        BENCH_RENDER=window $X/run.sh $app innerfling 4 $R/$app-innerfling-b-$round.json | tee -a $R/series.log
        $X/run.sh $app innerkeep 0 $R/$app-innerkeep-$round.json | tee -a $R/series.log
      fi
    else
      for k in $KINDS; do
        mkdir -p $R/$k
        BENCH_KINDS=$k $X/run.sh $app fling 0 $R/$k/$app-fling-t-$round.json | tee -a $R/series.log
        BENCH_KINDS=$k $X/run.sh $app ladder 0 $R/$k/$app-ladder-t-$round.json | tee -a $R/series.log
      done
    fi
    sleep 5
  done
done
log "series done"
