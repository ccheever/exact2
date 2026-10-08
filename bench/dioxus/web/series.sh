#!/bin/bash
# series.sh — the web comparison on headed Chrome: bench.mjs over the builds in dist/ (build.sh), interleaved rounds
# (ROUNDS=3; the order reverses every other round), then progress.mjs at 12k and 24k px/s for each. Results:
# <checkout>/target/bench/dioxus/results/web-<date>/ (web.json, a screenshot per app and round, progress.txt,
# provenance.txt). APPS="exact dioxus" (also bounded). Run with nothing else heavy on the machine; the load
# average goes into provenance.txt.
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../../.." && pwd)
O=${BENCH_RESULTS:-$ROOT/target/bench/dioxus/results}/web-$(date +%Y%m%d-%H%M); mkdir -p "$O"
APPS=${APPS:-exact dioxus}; SPECS=()
for a in $APPS; do [ -s "$H/dist/$a/index.html" ] || { echo "no dist/$a: run build.sh"; exit 1; }; SPECS+=("$a=$H/dist/$a"); done
CH=${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}
{ echo "$(date '+%F %T') checkout $(git -C "$ROOT" rev-parse --short=9 HEAD); apps $APPS; $("$CH" --version)"; uptime; } > "$O/provenance.txt"
(cd "$ROOT" && SPEEDS=${SPEEDS:-3000,6000,12000,24000} bun "$H/bench.mjs" "$O/web.json" "${SPECS[@]}")
for a in $APPS; do for s in 12000 24000; do
  echo "$a $(cd "$ROOT" && bun "$H/progress.mjs" "$H/dist/$a" $s 4)" | tee -a "$O/progress.txt"
done; done
python3 "$H/../summarize.py" web "$O/web.json"
