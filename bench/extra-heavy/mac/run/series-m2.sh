#!/bin/bash
# series-m2.sh — the mac lane's interleaved series, run ON the bench Mac from ~/xhm.
# SERIES=<name> (results/<name>/), APPS="swiftui exact2base", ROUNDS=3. The 19-kind feed's scenarios (fling-t,
# fling-live, rest, ladder-t, jump, coldstart, fling-b, innerfling-t, innerfling-b, innerkeep) and the live-only
# feed (fling-t, ladder-t under live/). PARTS="feed live" picks; SCEN="fling-t innerfling-t" limits the feed's.
# provenance.txt: each app's commit file (apps/<app>.commit), the machine, the OS, the date.
X=${BENCH_STAGE:-$HOME/xhm}; R=$X/results/${SERIES:-m2-r1}; mkdir -p $R/live
export BENCH_DELAY=${BENCH_DELAY:-15}
APPS=${APPS:-swiftui exact2base}; PARTS=${PARTS:-feed live}
ALL="fling-t fling-live rest ladder-t jump coldstart fling-b innerfling-t innerfling-b innerkeep"
SCEN=${SCEN:-$ALL}
log() { echo "$(date +%T) $*" | tee -a $R/series.log; }
{
  echo "series ${SERIES:-m2-r1} $(date)"; echo "machine $(hostname) $(sysctl -n machdep.cpu.brand_string) macOS $(sw_vers -productVersion)"
  for app in $APPS; do echo "app $app: $(cat $X/apps/$app.commit 2>/dev/null | tr '\n' ' ') env: $(cat $X/apps/$app.env 2>/dev/null | tr '\n' ' ')"; done
  echo "load $(uptime)"
} >> $R/provenance.txt
go() { # go <out.json> <run.sh args…>, env passed through; retried once if not a whole run
  local out=$1; shift
  local line; line=$($X/run.sh "$@" "$out"); echo "$line" | tee -a $R/series.log
  case "$line" in ok*) return 0;; esac
  log "RETRY $out (was: $line)"; mv -f "$out" "$out.failed" 2>/dev/null
  line=$($X/run.sh "$@" "$out"); echo "$line" | tee -a $R/series.log
  case "$line" in ok*) return 0;; esac
  log "FAILED TWICE $out"; return 1
}
has() { case " $SCEN " in *" $1 "*) return 0;; esac; return 1; }
# The order alternates each round (A B, B A, A B): an order effect shows as an odd/even split instead of hiding
# in the mean (PROGRAM.md rule 4, 2026-09-30: an iPad confirm read +19 ms/s with the tip second in every round).
# ORDER=fixed keeps the given order every round. The order each round ran is logged and in provenance.txt.
reversed() { local out=""; for a in $1; do out="$a $out"; done; echo $out; }
for round in $(seq 1 ${ROUNDS:-3}); do
  ROUND_APPS="$APPS"
  if [ "${ORDER:-alternate}" = alternate ] && [ $((round % 2)) -eq 0 ]; then ROUND_APPS="$(reversed "$APPS")"; fi
  log "round $round order: $ROUND_APPS"; echo "round $round order: $ROUND_APPS" >> $R/provenance.txt
  for app in $ROUND_APPS; do
    log "round $round $app"
    case " $PARTS " in *" feed "*)
      has fling-t && go $R/$app-fling-t-$round.json $app fling 0
      has fling-live && BENCH_LIVE=1 go $R/$app-fling-live-$round.json $app fling 0
      has rest && go $R/$app-rest-$round.json $app rest 0
      has ladder-t && go $R/$app-ladder-t-$round.json $app ladder 0
      has jump && BENCH_RENDER=window go $R/$app-jump-layer-$round.json $app jump 1
      has coldstart && BENCH_RENDER=window go $R/$app-coldstart-$round.json $app coldstart 1
      has fling-b && BENCH_RENDER=window go $R/$app-fling-b-$round.json $app fling 4
      has innerfling-t && go $R/$app-innerfling-t-$round.json $app innerfling 0
      has innerfling-b && BENCH_RENDER=window go $R/$app-innerfling-b-$round.json $app innerfling 4
      has innerkeep && go $R/$app-innerkeep-$round.json $app innerkeep 0
    ;; esac
    case " $PARTS " in *" live "*)
      BENCH_KINDS=live go $R/live/$app-fling-t-$round.json $app fling 0
      BENCH_KINDS=live go $R/live/$app-ladder-t-$round.json $app ladder 0
    ;; esac
    sleep 5
  done
done
echo "load at end $(uptime)" >> $R/provenance.txt
log "series done"
