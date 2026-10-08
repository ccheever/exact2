#!/bin/bash
# web.sh <app> <scenario> <sample> <out-name> — one web-pair probe run in Chrome (on the bench Mac, from this
# directory or a copy of it, with server.py already serving; series.sh starts it). <app>: exact2 (:8811) | expo (:8812).
# Chrome gets a private profile in profile/ beside this script (kept between runs, so map tiles come from its cache),
# a fixed 1366 x 940 window at the top-left, and flags that stop it throttling a background or occluded page.
# Its PID is recorded (chrome.pid, which server.py's /__mark reads) and only that process and its descendants
# are killed afterwards. The result lands in $RESULTS/<out-name> (default results/ beside this script).
W=$(cd "$(dirname "$0")" && pwd)
APP=$1; S=$2; SMP=$3; OUT=$4
RESULTS=${RESULTS:-$W/results}
case $APP in exact2) PORT=8811 ;; expo) PORT=8812 ;; *) echo "app: exact2 | expo"; exit 2 ;; esac
CHROME=${BENCH_CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
PROFILE=$W/profile; mkdir -p "$PROFILE" "$RESULTS"
rm -f "$RESULTS/$OUT"
/usr/bin/caffeinate -u -t 2   # wake the display: a sleeping display runs no rAF
URL="http://127.0.0.1:$PORT/?scenario=$S&sample=$SMP&out=$OUT&delay=${BENCH_DELAY:-8}&bench_notick=1"
"$CHROME" --user-data-dir="$PROFILE" --no-first-run --no-default-browser-check --disable-default-apps \
  --window-size=1366,940 --window-position=0,0 --disable-background-timer-throttling \
  --disable-renderer-backgrounding --disable-backgrounding-occluded-windows --enable-precise-memory-info --enable-logging=stderr --v=0 \
  --disable-features=CalculateNativeWinOcclusion --new-window "$URL" > "$RESULTS/$OUT.chrome.log" 2>&1 &
PID=$!
echo $PID > $W/chrome.pid
for i in $(seq 1 ${RUN_LIMIT:-150}); do
  [ -s "$RESULTS/$OUT" ] && break
  kill -0 $PID 2>/dev/null || break
  sleep 1
done
# Kill the recorded Chrome and everything under it (children found by parent PID, recursively).
tree() { echo $1; for c in $(/usr/bin/pgrep -P $1); do tree $c; done; }
PIDS=$(tree $PID)
kill $PID 2>/dev/null
for i in $(seq 1 20); do kill -0 $PID 2>/dev/null || break; sleep 0.25; done
for p in $PIDS; do kill -0 $p 2>/dev/null && kill -9 $p 2>/dev/null; done
rm -f $W/chrome.pid
[ -s "$RESULTS/$OUT" ] && { echo "ok $OUT"; exit 0; }
echo "timeout $OUT"; exit 1
