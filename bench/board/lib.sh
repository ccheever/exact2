# lib.sh — sourced by the board's series scripts (bench lane, 2026-09-30). The device rules in one place:
#   dev <iphone|ipad>     sets DEVN, U and BENCH_DEVICE (BENCH_IPHONE / BENCH_IPAD, else BENCH_DEVICE) and LK, the
#                         lock command (BENCH_LOCK, else this directory's devlock.sh <dev>)
#   take <who> / give     the queued device lock; every take runs the probe's devclean.sh; every exit gives the lock back
#   inst <app.app>…       install under the lock (extra-heavy's ios/install.sh: 180 s cap, logs stamp + UUID); a failure
#                         gives the lock and exits 1
#   drun <devrun args…>   one probe run, RUN_CAP (420 s) wall-clock cap; "launch failed" is retried twice; a timed-out
#                         or partial run is retried once (a partial result is kept as <out>.partial, never summarized);
#                         two runs in a row that cannot launch give the lock and exit 3 (the device stopped answering)
#   crash <dir>           crash logs of the bench apps since this script started, copied into <dir>
# State (priority files, thermal log and readings, build markers) lives in BENCH_STATE (default
# <checkout>/target/bench/board).
export LOCK_HOLDER_PID=$$
BD=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); ROOT=$(cd "$BD/../.." && pwd)
P=${BENCH_PROBE:-$ROOT/bench/heavy-list/probe}; D=${DEVRUN:-$P/devrun.sh}
INSTALL_SH=$ROOT/bench/extra-heavy/ios/install.sh
STATE=${BENCH_STATE:-$ROOT/target/bench/board}; mkdir -p "$STATE"
SINCE=${CRASH_SINCE:-$(date +%Y-%m-%d-%H%M%S)}
HELD=; WHO=; FAILS=0
dev() {
  DEVN=$1
  case "$1" in
    iphone) U=${BENCH_IPHONE:-$BENCH_DEVICE} ;;
    ipad) U=${BENCH_IPAD:-$BENCH_DEVICE} ;;
    *) U=$BENCH_DEVICE ;;
  esac
  [ -n "$U" ] || { echo "set BENCH_IPHONE / BENCH_IPAD (or BENCH_DEVICE) to the $1's UDID (xcrun devicectl list devices)"; exit 1; }
  export BENCH_DEVICE=$U
  LK=${BENCH_LOCK:-$BD/devlock.sh $DEVN}
}
# priority: $STATE/prio-<dev> when it exists (read at every take), else LOCK_PRIO, else 5
take() {
  WHO=$1
  while :; do
    if [ -n "$GATE" ] && [ -n "$LASTGIVE" ]; then  # at least COOL seconds between the previous give and this take
      local left=$(( LASTGIVE + ${COOL:-300} - $(date +%s) )); [ $left -gt 0 ] && { echo "$(date +%T) cool-down: $left s since the last give"; sleep $left; }
    fi
    LOCK_PRIO=$(cat $STATE/prio-$DEVN 2>/dev/null || echo ${LOCK_PRIO:-5}) $LK take $WHO; HELD=1; "$P/devclean.sh"
    [ -z "$GATE" ] && return
    local s; s=$(ping_thermal)
    echo "$(date '+%F %T') $DEVN gate before $WHO: thermal $s" | tee -a $STATE/thermal-$DEVN.log
    case "$s" in 0|1) return ;; esac
    # serious, critical or no reading: the static list stays in front, the lock goes back, the phone cools
    give; echo "$(date +%T) cooling ${COOL:-300} s (thermal $s)"; sleep ${COOL:-300}
  done
}
# GATE=1 (the iPhone, the coordinator's thermal ruling of 2026-09-30 18:35): a take (one app's scenarios, <= 10 min)
# starts only when the probe's own reading is 0 (nominal) or 1 (fair) — a `rest` run of the static UIKit heavy list
# (BENCH_PING_APP, ~20 s), which then stays in front as the keeper — and at least COOL (300) s after the previous
# hold's give. Every result carries the state it ran at (the probe's fields; also logged per run to
# $STATE/thermal-<dev>.log) and the scoreboard does not rank a metric whose apps ran at different states; the app order
# rotates per round so each app meets the same states. Nothing is set aside for heat.
PINGAPP=${BENCH_PING_APP:-$ROOT/bench/heavy-list/uikit/build/HeavyUIKit.app}; PINGID=${BENCH_PING_ID:-dev.exact.heavybench.uikit}; PINGED=
worst_thermal() { python3 -c "
import json,sys
try: r=json.load(open(sys.argv[1]))
except Exception: print('-'); sys.exit()
v=[r.get('thermalStart'),r.get('thermalEnd')]+[x.get('thermal') for x in r.get('segStats',[])]+[c[1] for c in (r.get('thermalChanges') or [])]
v=[x for x in v if isinstance(x,(int,float)) and not isinstance(x,bool) and x>=0]
print(int(max(v)) if v else '-')" "$1"; }
ping_thermal() {
  local o=$STATE/thermal/$DEVN-$(date +%m%d-%H%M%S).json; mkdir -p $STATE/thermal
  [ -z "$PINGED" ] && { "$INSTALL_SH" $U "$PINGAPP" >/dev/null 2>&1; PINGED=1; }
  BENCH_DELAY=1 timeout ${RUN_CAP:-420} "$D" $PINGID rest 0 $o >/dev/null 2>&1
  [ -s $o ] && worst_thermal $o || echo "-"
}

give() { [ -n "$HELD" ] && { $LK give $WHO; LASTGIVE=$(date +%s); }; HELD=; }
trap give EXIT; trap 'exit 1' INT TERM HUP
inst() { "$INSTALL_SH" $U "$@" || { give; echo "aborted: install failed"; exit 1; }; }
drun() {
  [ -n "$SKIP_DONE" ] && [ -s "${@: -1}" ] && return
  local out o="${@: -1}" again=1
  for t in 1 2 3; do
    out=$(timeout ${RUN_CAP:-420} "$D" "$@"); [ -z "$out" ] && out="launch failed"  # a devrun the cap killed prints nothing
    echo "$out"
    case "$out" in
      "launch failed") sleep 3; continue ;;
      ok*) FAILS=0
           [ -n "$GATE" ] && echo "$(date '+%F %T') run $(basename "$o"): worst thermal $(worst_thermal "$o")" >> $STATE/thermal-$DEVN.log
           return ;;
      *) [ -s "$o" ] && mv "$o" "$o.partial"
         [ $again = 1 ] || { FAILS=0; return; }; again=0; echo "$(date +%T) retrying once: $out"; sleep 3 ;;
    esac
  done
  [ "$out" = "launch failed" ] || { FAILS=0; return; }
  FAILS=$((FAILS+1))
  if [ $FAILS -ge 2 ]; then echo "$(date +%T) aborted: $DEVN stopped launching (2 runs, 3 tries each); lock given back"; give; exit 3; fi
}
crash() { "$P/crashcheck.sh" $SINCE "$1"; }
