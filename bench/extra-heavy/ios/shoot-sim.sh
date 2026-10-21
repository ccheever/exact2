#!/bin/sh
# usage: shoot-sim.sh <app: swiftui|uikit|expo|exact2> <name> <delay-s> [ENV=VAL…] — relaunch dev.exact.xheavy.<app> on
# the simulator BENCH_SIM (a UDID; an iPad Pro 13-inch matches the device runs) and screenshot it to
# $BENCH_SHOTS/sim/<app>-<name>.png (default <checkout>/target/bench/extra-heavy/shots).
H=$(cd "$(dirname "$0")" && pwd); U=${BENCH_SIM:?set BENCH_SIM to a simulator UDID}; a=$1; n=$2; d=$3; shift 3
O=${BENCH_SHOTS:-$H/../../../target/bench/extra-heavy/shots}/sim; mkdir -p "$O"
xcrun simctl terminate $U dev.exact.xheavy.$a 2>/dev/null
env $(for kv in "$@"; do printf 'SIMCTL_CHILD_%s ' "$kv"; done) xcrun simctl launch $U dev.exact.xheavy.$a >/dev/null
perl -e "select(undef,undef,undef,$d)"
xcrun simctl io $U screenshot "$O/$a-$n.png" >/dev/null 2>&1; echo "$O/$a-$n.png"
