#!/bin/sh
# usage: shoot.sh <name> <delay-s> [ENV=VAL...] — relaunch on the simulator BENCH_SIM and screenshot to
# <checkout>/target/bench/heavy-list/shots/swiftui-<name>.png
cd "$(dirname "$0")"; U=${BENCH_SIM:?set BENCH_SIM to a simulator udid}; n=$1; d=$2; shift 2
O=../../../target/bench/heavy-list/shots; mkdir -p $O
xcrun simctl terminate $U dev.exact.heavybench.swiftui 2>/dev/null
env $(for kv in "$@"; do printf 'SIMCTL_CHILD_%s ' "$kv"; done) xcrun simctl launch $U dev.exact.heavybench.swiftui >/dev/null
perl -e "select(undef,undef,undef,$d)"
xcrun simctl io $U screenshot $O/swiftui-$n.png >/dev/null 2>&1; echo $O/swiftui-$n.png
