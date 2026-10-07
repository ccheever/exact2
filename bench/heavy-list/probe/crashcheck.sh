#!/bin/bash
# crashcheck.sh <since YYYY-MM-DD-HHMMSS> <outdir> — list the crash logs of the device BENCH_DEVICE, print every bench
# app entry (the exact2, SwiftUI, UIKit and Expo bench executables, JetsamEvent) stamped at or after <since>, and copy
# new ones into <outdir>. A run that printed "ok" may still have crashed and relaunched; read these before trusting a
# series. The caller holds the device lock, if any. Prints "crash logs: none …" when clean.
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}
SINCE=$1; OUT=$2
# Crash log names start with the process name, which for the exact2 app has a space ("Heavy list-2026-…").
names=$(timeout 60 xcrun devicectl device info files --device $U --domain-type systemCrashLogs 2>/dev/null \
  | grep -oE '(ExactIOS|Heavy list|HeavyBench|HeavyUIKit|HeavyBenchExpo|XHeavy[A-Za-z ]*|JetsamEvent)-[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}[^ ]*' )
n=0
IFS=$'\n'
for f in $names; do
  ts=$(echo "$f" | sed -E 's/^[A-Za-z ]+-([0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}).*/\1/')
  [[ "$ts" < "$SINCE" ]] && continue
  n=$((n+1)); mkdir -p "$OUT"
  if [ ! -s "$OUT/$f" ]; then
    timeout 60 xcrun devicectl device copy from --device $U --domain-type systemCrashLogs --source "$f" --destination "$OUT/$f" >/dev/null 2>&1
    echo "CRASH LOG (new) $f -> $OUT/$f"
  fi
done
[ $n = 0 ] && echo "crash logs: none since $SINCE" || echo "crash logs: $n since $SINCE (in $OUT)"
