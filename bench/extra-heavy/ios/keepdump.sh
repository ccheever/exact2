#!/bin/bash
export LOCK_HOLDER_PID=$$  # for a BENCH_LOCK that reclaims the lock when its holder dies
# keepdump.sh — one innerkeep per app on BENCH_DEVICE with the anchor images saved (BENCH_JUMPDUMP), to check the
# "?" (no matching shift) cases of summarize.py's innerkeep column by eye. INSTALL="<app.app> …" installs first.
# Out: $BENCH_RESULTS/keepdump/<app>.json, <app>.log and <app>-kd/.
H=$(cd "$(dirname "$0")" && pwd)
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}
O=${BENCH_RESULTS:-$H/../../../target/bench/extra-heavy/results}/keepdump; mkdir -p $O
[ -n "$BENCH_LOCK" ] && $BENCH_LOCK take keepdump
if [ -n "$INSTALL" ]; then $H/install.sh $U $INSTALL || { [ -n "$BENCH_LOCK" ] && $BENCH_LOCK give keepdump; exit 1; }; fi
for app in ${APPS:-swiftui expo exact2 uikit}; do
  BENCH_DELAY=15 BENCH_KINDS= BENCH_JUMPDUMP=kd DBG_SECS=200 $H/dbgrun.sh dev.exact.xheavy.$app innerkeep 0 $O/$app.json $O/$app.log
done
[ -n "$BENCH_LOCK" ] && $BENCH_LOCK give keepdump
