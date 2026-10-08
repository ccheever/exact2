#!/bin/bash
# run.sh <app> <scenario> <sample> <out.json> — one probe run of an app on this Mac, from the stage directory
# BENCH_STAGE (default ~/xhm; ../push.sh fills it), locally or over ssh. <app>: swiftui | exact2. Passes BENCH_RENDER, BENCH_DUMP, BENCH_DUMPALL,
# BENCH_DELAY, BENCH_LIVE, BENCH_KINDS, BENCH_FREEZE, BENCH_START_INDEX, BENCH_METALDBG through.
# The app is launched through Launch Services (`open -n --env`), so it runs in the logged-in GUI session
# whatever the ssh session is; its PID is read once by its unique path under ~/xhm and killed at the
# end (the probe also exits by itself with BENCH_EXIT=1). Nothing else is ever killed.
X=${BENCH_STAGE:-$HOME/xhm}
APP=$1; S=$2; SMP=$3; OUT=$4
case $APP in
  swiftui) BUNDLE=$X/apps/XHeavy.app ;;
  exact2*) BUNDLE=$X/apps/$APP.app ;;   # exact2, or another exact2 build (exact2base, exact2mb): ../push.sh stages them
  *) echo "app: swiftui | exact2 | exact2<name>"; exit 2 ;;
esac
[ -d "$BUNDLE" ] || { echo "no $BUNDLE"; exit 2; }
EXE=$BUNDLE/Contents/MacOS/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$BUNDLE/Contents/Info.plist")
W=${BENCH_WIN_W:-1366}; H=${BENCH_WIN_H:-940}
rm -f "$OUT" "$OUT.partial" "$OUT.log" "$OUT.state"
# Wake the display and keep it on for the run: a sleeping display ticks no display link. The user-activity
# assertion is HELD for the run (its PID recorded, killed at the end). A 2 s one was not enough on a locked Mac
# whose display had been off: the lock screen turns a display woken that way off again 12 s later, whatever
# display-sleep assertion the probe holds (bones, 2026-09-30 08:10: on :20, off :32, 4 s into the fling; the app
# then sat without frames until RUN_LIMIT and the result was a 2 s `partial`). The first run of a series did that.
LIMIT=${RUN_LIMIT:-150}
# The machine's state, before and after every run (2026-09-30: the M1's CPU speed swung 3-5x within minutes and a
# whole series was taken in the slow state). tmp/speed.py reads loops/s of a fixed loop for one process and per
# process at four. ~/xhm/machine.state names this machine's floor (MIN1=… MIN4=…; bones 760/730, the M1 500/470:
# about 90 % of what each reads idle). The launch WAITS for the floor (10 s steps, STATE_WAIT seconds at most,
# default 180) and <out>.state records what was read; check-m2.py refuses a result read below the floor before or
# after. STATE_GATE=off records without waiting (a deliberately loaded machine).
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
MIN1=0; MIN4=0; [ -f "$X/machine.state" ] && . "$X/machine.state"
mstate() { $PY $X/tmp/speed.py 2>/dev/null || echo "speed1 0 speed4 0"; }
WAITED=0
while :; do
  ST=$(mstate); S1=$(echo $ST | awk '{print $2}'); S4=$(echo $ST | awk '{print $4}')
  [ "${STATE_GATE:-on}" = off ] && break
  [ "${S1:-0}" -ge "$MIN1" ] && [ "${S4:-0}" -ge "$MIN4" ] && break
  [ $WAITED -ge ${STATE_WAIT:-180} ] && break
  sleep 10; WAITED=$((WAITED + 12))
done
echo "before $ST waited $WAITED min1 $MIN1 min4 $MIN4 gate ${STATE_GATE:-on} load $(sysctl -n vm.loadavg) at $(date +%T)" > "$OUT.state"
/usr/bin/caffeinate -u -t $((LIMIT + 30)) & CAFF=$!; disown $CAFF 2>/dev/null
# PROBE=<file under ~/xhm> inserts another build of the probe (a diagnostic one); the series use probe-mac.dylib.
ARGS=(--env DYLD_INSERT_LIBRARIES=$X/${PROBE:-probe-mac.dylib} --env BENCH_SCENARIO=$S --env BENCH_SAMPLE=$SMP
      --env BENCH_OUT=$OUT --env BENCH_EXIT=1 --env BENCH_WIN_W=$W --env BENCH_WIN_H=$H
      --env EXACT_WINDOW_WIDTH=$W --env EXACT_WINDOW_HEIGHT=$H)
for v in BENCH_VM BENCH_MAPCOUNT BENCH_BLANKSCALE BENCH_DUMPFULL BENCH_RENDER BENCH_DUMP BENCH_DUMPALL BENCH_DELAY BENCH_LIVE BENCH_KINDS BENCH_FREEZE BENCH_START_INDEX BENCH_METALDBG; do
  [ -n "${!v}" ] && ARGS+=(--env "$v=${!v}")
done
# An app's own switches (one build measured two ways): apps/<app>.env, KEY=VALUE per line, each passed to the app.
# series-m2.sh copies the file into provenance.txt.
if [ -f "$X/apps/$APP.env" ]; then
  while IFS= read -r kv; do case "$kv" in ''|'#'*) ;; *=*) ARGS+=(--env "$kv") ;; esac; done < "$X/apps/$APP.env"
fi
/usr/bin/open -n -F --stdout "$OUT.log" --stderr "$OUT.log" "${ARGS[@]}" "$BUNDLE" || { echo "launch failed"; exit 1; }
PID=""
for i in $(seq 1 50); do PID=$(/usr/bin/pgrep -n -f "^$EXE") && break; sleep 0.1; done
echo "$PID" > $X/tmp/running.pid
# The first launch of a series has come up visible but not active/key twice today (its result refused by
# check-m2.py): a second `open` without -n activates the running instance through Launch Services (no TCC).
sleep 1; /usr/bin/open "$BUNDLE" 2>/dev/null
for i in $(seq 1 $LIMIT); do
  [ -s "$OUT" ] && break
  [ -n "$PID" ] && ! kill -0 $PID 2>/dev/null && break
  sleep 1
done
# Give the probe's exit a moment, then make sure the recorded PID is gone.
for i in 1 2 3 4 5; do [ -n "$PID" ] && kill -0 $PID 2>/dev/null || break; sleep 0.5; done
if [ -n "$PID" ] && kill -0 $PID 2>/dev/null; then kill $PID; sleep 1; kill -0 $PID 2>/dev/null && kill -9 $PID; fi
rm -f $X/tmp/running.pid
kill $CAFF 2>/dev/null
echo "after $(mstate) load $(sysctl -n vm.loadavg) at $(date +%T)" >> "$OUT.state"
if [ -s "$OUT" ]; then rm -f "$OUT.partial"; echo "ok $OUT"; exit 0; fi
# A partial result is kept for reading but is NOT a run: exit 3, and check-m2.py refuses it in a series.
[ -s "$OUT.partial" ] && { mv "$OUT.partial" "$OUT"; echo "partial $OUT"; exit 3; }
echo "timeout $OUT (pid $PID)"; exit 1
