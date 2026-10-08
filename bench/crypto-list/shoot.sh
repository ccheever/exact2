#!/bin/sh
# usage: shoot.sh <app: swiftui|uikit|expo|svgi|gpu> <name> <delay-s> [ENV=VAL...] — relaunch dev.exact.cryptobench.<app> on the
# simulator BENCH_SIM and screenshot to <checkout>/target/bench/crypto-list/shots/<app>-<name>.png. The app must be installed
# (xcrun simctl install $BENCH_SIM <app>/build-sim/<App>.app, or build.mjs --ios for exact2). Parity shots: BENCH_FREEZE=1, and BENCH_START_INDEX=500 for "after a jump".
cd "$(dirname "$0")"; U=${BENCH_SIM:?set BENCH_SIM to a simulator udid}; a=$1; n=$2; d=$3; shift 3
O=../../target/bench/crypto-list/shots; mkdir -p $O
xcrun simctl terminate $U dev.exact.cryptobench.$a 2>/dev/null
env $(for kv in "$@"; do printf 'SIMCTL_CHILD_%s ' "$kv"; done) xcrun simctl launch $U dev.exact.cryptobench.$a >/dev/null
perl -e "select(undef,undef,undef,$d)"
xcrun simctl io $U screenshot $O/$a-$n.png >/dev/null 2>&1; echo $O/$a-$n.png
