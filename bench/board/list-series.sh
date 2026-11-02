#!/bin/bash
# list-series.sh <xheavy|crypto|heavy> <iphone|ipad> — one benchmark's interleaved rounds on one device for the board,
# the device lock taken per app (LOCK_PRIO default 5), devclean + that app's install at every take (lib.sh).
# SERIES=<name> (required), ROUNDS=3, FROM=<round>, SKIP_DONE=1 resumes, GATE=1 gates every take on the thermal state.
# INSTALL_MAP="<app>=<bundle.app> …" (required): the bundle installed for each app id suffix.
# Results: $BENCH_TARGET/<bench>/results/<SERIES>/ (BENCH_TARGET default <checkout>/target/bench), the layout each
# bench's own series writes, so its summarize.py reads them:
#   xheavy: extra-heavy, dev.exact.xheavy.{swiftui,uikit,exact2}; the scenarios of bench/extra-heavy/ios/series.sh
#           (19 kinds). <name>@<series> in APPS is a second build of app <name> in the same rotation (INSTALL_MAP's
#           <name>@<series> entry), its results in results/<series>/.
#   crypto: crypto-list, dev.exact.cryptobench.{swiftui,uikit,svgi}; the extra-heavy scenarios without the inner lists.
#   heavy:  heavy-list, dev.exact.heavybench.{swiftui,uikit,exact}; the scenarios of bench/heavy-list/series.sh.
# APPS and BENCH_PREFIX (the bundle id prefix) override the defaults above.
source "$(dirname "$0")/lib.sh"
B=${1:?bench}; dev ${2:?device}
T=${BENCH_TARGET:-$ROOT/target/bench}
case $B in
  crypto) PRE=dev.exact.cryptobench; R=$T/crypto-list/results/${SERIES:?}; APPS=${APPS:-swiftui uikit svgi}; export BENCH_DELAY=${BENCH_DELAY:-15} ;;
  heavy)  PRE=dev.exact.heavybench;  R=$T/heavy-list/results/${SERIES:?};  APPS=${APPS:-swiftui uikit exact} ;;
  xheavy) PRE=dev.exact.xheavy; R=$T/extra-heavy/results/${SERIES:?}; APPS=${APPS:-swiftui uikit exact2}; export BENCH_DELAY=${BENCH_DELAY:-15} BENCH_KINDS= ;;
  *) echo "bench?"; exit 1 ;;
esac
PRE=${BENCH_PREFIX:-$PRE}
mkdir -p $R; R0=$R
bundle() { for kv in ${INSTALL_MAP:?}; do [ "${kv%%=*}" = "$1" ] && echo "${kv#*=}"; done; }
# The app order rotates each round (the cpu lane measured ~19 ms/s CPU from a fixed position):
# round 1 as given, round 2 starting from the second app, round 3 from the third. ROTATE=0 keeps the given order.
rot() { local n=$1; shift; local a=("$@"); local k=$(( (n - 1) % ${#a[@]} )); [ "${ROTATE:-1}" = 0 ] && k=0; echo "${a[@]:$k} ${a[@]:0:$k}"; }
for round in $(seq ${FROM:-1} ${ROUNDS:-3}); do
  for full in $(rot $round $APPS); do
    # <name>@<series> (xheavy): a second build of app <name> in the same rotation; results in results/<series>/
    app=${full%%@*}; RR=$R0; [ "$full" != "$app" ] && { RR=$(dirname $R0)/${full#*@}; mkdir -p $RR; }
    R=$RR
    last=$R/$app-fling-b-$round.json; [ $B = heavy ] && last=$R/$app-rest-$round.json; [ $B = xheavy ] && last=$R/$app-innerkeep-$round.json
    [ -n "$SKIP_DONE" ] && [ -s $last ] && continue
    take board-$B; inst $(bundle $full)
    ID=$PRE.$app
    echo "$(date +%T) round $round $full"
    if [ $B = xheavy ]; then
      drun $ID fling 0 $R/$app-fling-t-$round.json
      BENCH_LIVE=1 drun $ID fling 0 $R/$app-fling-live-$round.json
      drun $ID rest 0 $R/$app-rest-$round.json
      drun $ID ladder 0 $R/$app-ladder-t-$round.json
      BENCH_RENDER=layer drun $ID jump 1 $R/$app-jump-layer-$round.json
      BENCH_RENDER=layer drun $ID coldstart 1 $R/$app-coldstart-$round.json
      BENCH_RENDER=layer drun $ID fling 4 $R/$app-fling-b-$round.json
      drun $ID innerfling 0 $R/$app-innerfling-t-$round.json
      BENCH_RENDER=layer drun $ID innerfling 4 $R/$app-innerfling-b-$round.json
      drun $ID innerkeep 0 $R/$app-innerkeep-$round.json
    elif [ $B = crypto ]; then
      drun $ID fling 0 $R/$app-fling-t-$round.json
      BENCH_LIVE=1 drun $ID fling 0 $R/$app-fling-live-$round.json
      drun $ID rest 0 $R/$app-rest-$round.json
      drun $ID ladder 0 $R/$app-ladder-t-$round.json
      BENCH_RENDER=layer drun $ID jump 1 $R/$app-jump-layer-$round.json
      BENCH_RENDER=layer drun $ID coldstart 1 $R/$app-coldstart-$round.json
      BENCH_RENDER=layer drun $ID fling 4 $R/$app-fling-b-$round.json
    else
      drun $ID fling 0 $R/$app-fling-t-$round.json
      BENCH_RENDER=layer drun $ID fling 4 $R/$app-fling-b-$round.json
      drun $ID ladder 0 $R/$app-ladder-t-$round.json
      BENCH_RENDER=layer drun $ID ladder 4 $R/$app-ladder-b-$round.json
      BENCH_RENDER=layer drun $ID jump 1 $R/$app-jump-layer-$round.json
      BENCH_RENDER=layer drun $ID coldstart 1 $R/$app-coldstart-$round.json
      BENCH_LIVE=1 drun $ID fling 0 $R/$app-fling-live-$round.json
      drun $ID rest 0 $R/$app-rest-$round.json
    fi
    crash $R0/crash; give
  done
  sleep 10
done
echo series done
