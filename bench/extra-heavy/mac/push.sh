#!/bin/bash
# push.sh <label|swiftui> [ssh-host…] — stage an app and the current runner scripts on the bench Mac: into
# BENCH_STAGE (default ~/xhm) on each ssh host given (or BENCH_HOST), or on this Mac when neither names one.
#   <label>   dist/<label>.app (build-exact.sh; "main" for an empty label) → apps/exact2<label>.app, with
#             exact2<label>.commit beside it (series-m2.sh writes it into provenance.txt); label "main" → apps/exact2.app
#   swiftui   swiftui/build/XHeavy.app → apps/XHeavy.app
# Also copies run/*.sh and run/mstate/*.sh to the stage, run/tp/* and run/mstate/speed.py to its tmp/, and probe/probe-mac.dylib.
set -e
L=${1:?label or swiftui}; shift
H=$(cd "$(dirname "$0")" && pwd); S=${BENCH_STAGE:-'~/xhm'}
case $L in
  swiftui) SRC=$H/swiftui/build/XHeavy.app; NAME=XHeavy; COMMIT= ;;
  main)    SRC=$H/dist/main.app; NAME=exact2; COMMIT=$H/dist/main.commit ;;
  *)       SRC=$H/dist/$L.app; NAME=exact2$L; COMMIT=$H/dist/$L.commit ;;
esac
[ -d "$SRC" ] || { echo "no $SRC"; exit 1; }
[ -s "$H/probe/probe-mac.dylib" ] || { echo "no probe/probe-mac.dylib: run probe/build.sh"; exit 1; }
hosts=("$@"); [ ${#hosts[@]} = 0 ] && [ -n "$BENCH_HOST" ] && hosts=("$BENCH_HOST")
stage() { # <dest prefix: "" locally, "host:" over ssh> <stage dir as that side spells it>
  local p=$1 s=$2
  rsync -a --delete "$SRC/" "$p$s/apps/$NAME.app/"
  [ -n "$COMMIT" ] && rsync -a "$COMMIT" "$p$s/apps/$NAME.commit"
  rsync -a "$H"/run/*.sh "$H"/run/mstate/*.sh "$H/probe/probe-mac.dylib" "$p$s/"
  rsync -a "$H"/run/tp/ "$H/run/mstate/speed.py" "$p$s/tmp/"
}
if [ ${#hosts[@]} = 0 ]; then
  X=$(eval echo "$S"); mkdir -p "$X/apps" "$X/tmp" "$X/results"; stage "" "$X"; echo "staged $NAME in $X"
else
  for h in "${hosts[@]}"; do
    ssh -o BatchMode=yes $h "mkdir -p $S/apps $S/tmp $S/results"
    X=$(ssh -o BatchMode=yes $h "cd $S && pwd"); stage "$h:" "$X"; echo "staged $NAME on $h:$X"
  done
fi
