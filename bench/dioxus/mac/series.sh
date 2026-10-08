#!/bin/bash
# series.sh [outdir] — macOS series: exact2 vs Dioxus Native, the heavy list. Per round and app (the order reverses
# every other round): macbench cold, then launch, place the window at 420 × 900, scroll at SPEEDS for 3 s each,
# screenshot, quit. Kills only the PIDs it started. Default outdir <checkout>/target/bench/dioxus/results/mac-<date>.
#   ROUNDS=3   SPEEDS=3000,6000,12000,24000   APPS="exact dioxus" (also bounded)
# Needs build.sh's tools and apps (apps.sh finds them) and the terminal's Accessibility, post-event and Screen
# Recording permissions (./tcc prints them).
set -u
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../../.." && pwd)
source "$H/apps.sh"
OUT=${1:-$ROOT/target/bench/dioxus/results/mac-$(date +%Y%m%d-%H%M)}; ROUNDS=${ROUNDS:-3}; SPEEDS=${SPEEDS:-3000,6000,12000,24000}
APPS=${APPS:-exact dioxus}
mkdir -p "$OUT"
exe() { case $1 in exact) echo "$EX";; bounded) echo "$BX";; *) echo "$DX_APP";; esac; }
assets() { case $1 in exact) echo "$EXA";; bounded) echo "$BXA";; esac; }
for a in $APPS; do [ -x "$(exe $a)" ] || { echo "no $a build ($(exe $a)): run build.sh"; exit 1; }; done
echo "$(date '+%F %T') checkout $(git -C "$ROOT" rev-parse --short=9 HEAD); apps $APPS; $(uptime)" >> "$OUT/provenance.txt"
launch() { # app -> pid
  local A; A=$(assets $1)
  if [ -n "$A" ]; then EXACT_ASSETS=$A "$(exe $1)" >/dev/null 2>&1 & else "$(exe $1)" >/dev/null 2>&1 & fi
  echo $!
}
envs() { local A; A=$(assets $1); [ -z "$A" ] || echo "EXACT_ASSETS=$A"; }
# Warm each once (dyld, page cache).
for a in $APPS; do p=$(launch $a); sleep 4; kill $p; wait $p 2>/dev/null; done
for r in $(seq 1 $ROUNDS); do
  if [ $((r % 2)) = 1 ]; then order="$APPS"; else order=$(echo $APPS | tr ' ' '\n' | tail -r | tr '\n' ' '); fi
  for a in $order; do
    "$H/macbench" cold "$OUT/$a-cold-$r.json" "$(exe $a)" $(envs $a) | tail -1
    sleep 1
    p=$(launch $a); echo "$a round $r pid $p"
    sleep 4
    "$H/macbench" place $p 60 40 420 900; sleep 2
    MACBENCH_HID=1 "$H/macbench" scroll $p "$OUT/$a-scroll-$r.json" $SPEEDS 3
    "$H/macbench" shot $p "$OUT/$a-end-$r.png" >/dev/null
    kill $p; wait $p 2>/dev/null
    sleep 2
  done
done
python3 "$H/../summarize.py" mac "$OUT"
