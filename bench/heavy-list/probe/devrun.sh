#!/bin/bash
# devrun.sh <bundle-id> <scenario> <sample> <local-out.json> — one probe run on the device BENCH_DEVICE (a UDID).
# The app must be installed with the probe embedded (resign.sh). The probe is loaded with DYLD_INSERT_LIBRARIES,
# unless BENCH_INSERT=0 (devrun-cold.sh: the makecold.sh copies, whose executable links it instead).
# Passes BENCH_RENDER and BENCH_DUMP through, the probe's and the apps' other BENCH_* switches when set, and any
# KEY=VALUE pairs in BENCH_ENV (comma-separated) for an app's own diagnostics.
# Prints "ok <out>", "partial <out> (app alive: n)", "timeout <out> (app alive: n)" or "launch failed".
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID (xcrun devicectl list devices)}; BID=$1; S=$2; SMP=$3; OUT=$4
# The app's container, read into a fresh temporary file (two devices may run at once; a failed query must not
# leave a stale container URL behind).
J=$(mktemp -t devrun-app); trap 'rm -f "$J"' EXIT
timeout ${BENCH_T_INFO:-60} xcrun devicectl device info apps --device $U --bundle-id $BID --json-output $J >/dev/null 2>&1
URL=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['result']['apps'][0]['url'].replace('file://',''))" $J 2>/dev/null)
[ -n "$URL" ] || { echo "launch failed"; exit 1; }
NAME="r-$(date +%s).json"
EXTRA=""
for v in BENCH_LIVE BENCH_HUD BENCH_DELAY BENCH_VM BENCH_FEATURE BENCH_FREEZE BENCH_INKDBG BENCH_KINDS BENCH_REST \
         BENCH_MALLOCLOG BENCH_METALDBG BENCH_JUMPDUMP BENCH_JUMPDUMP_ALL BENCH_START_INDEX BENCH_CHUNK BENCH_TIMING; do
  [ -n "${!v}" ] && EXTRA="$EXTRA,\"$v\":\"${!v}\""
done
IFS=, read -ra KV <<< "${BENCH_ENV:-}"
for kv in "${KV[@]}"; do [ -n "$kv" ] && EXTRA="$EXTRA,\"${kv%%=*}\":\"${kv#*=}\""; done
INSERT=""
[ "${BENCH_INSERT:-1}" = 0 ] || INSERT="\"DYLD_INSERT_LIBRARIES\":\"${URL}Frameworks/probe.dylib\","
ENV="{$INSERT\"BENCH_SCENARIO\":\"$S\",\"BENCH_SAMPLE\":\"$SMP\",\"BENCH_OUT\":\"$NAME\",\"BENCH_DUMP\":\"${BENCH_DUMP:-}\",\"BENCH_RENDER\":\"${BENCH_RENDER:-}\"$EXTRA}"
# Watchdog: a devicectl call that hangs (an iPad once held the device for 2 h 22 min on a launch that never
# returned) is killed: launch 90 s, info/copy 60 s each (BENCH_T_INFO / BENCH_T_LAUNCH raise the info and launch
# caps: single devicectl calls have taken up to two minutes on an iPhone just after its unlock).
timeout ${BENCH_T_LAUNCH:-90} xcrun devicectl device process launch --device $U --terminate-existing --environment-variables "$ENV" $BID >/dev/null 2>&1 || { echo "launch failed"; exit 1; }
rm -f "$OUT"
for i in $(seq 1 30); do
  sleep 4
  timeout 60 xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME" --destination "$OUT" >/dev/null 2>&1 && [ -s "$OUT" ] && break
done
if [ -s "$OUT" ]; then echo "ok $OUT"; exit 0; fi
# The app died or hung: keep its last per-segment checkpoint, marked partial.
timeout 60 xcrun devicectl device copy from --device $U --domain-type appDataContainer --domain-identifier $BID --source "Documents/$NAME.partial" --destination "$OUT" >/dev/null 2>&1
alive=$(timeout 60 xcrun devicectl device info processes --device $U 2>/dev/null | grep -c "/$(basename "${URL%/}")/")
if [ -s "$OUT" ]; then echo "partial $OUT (app alive: $alive)"; else echo "timeout $OUT (app alive: $alive)"; fi
