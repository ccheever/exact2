#!/bin/bash
# wait.sh [seconds] — the board's poll: wait (default 560 s) for an event in a chain's log (a hold given back, a step
# line, a gate reading, a stop), then print where each device is: its lock holder and queue, the chain log's last
# step lines, the last thermal readings. Logs: $STATE/chain-<dev>.log (the README's way of starting a chain) and
# $STATE/thermal-<dev>.log. DEVICES="iphone ipad" names the devices (and their locks: this directory's devlock.sh).
BD=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$BD/../.." && pwd); STATE=${BENCH_STATE:-$ROOT/target/bench/board}
T=${1:-560}; end=$(( $(date +%s) + T ))
DEVS=${DEVICES:-iphone ipad}
logs() { for d in $DEVS; do cat $STATE/chain-$d.log $STATE/thermal-$d.log 2>/dev/null; done; }
sig() { logs | grep -cE "free$|^[0-9:]+ ($(echo $DEVS | tr ' ' '|')): |aborted|gate before|cooling"; }
s=$(sig)
while [ $(date +%s) -lt $end ] && [ "$(sig)" = "$s" ]; do sleep 10; done
echo "== $(date +%T)"
for d in $DEVS; do
  echo "-- $d: lock $($BD/devlock.sh $d who | tr '\n' ' ') queue: $($BD/devlock.sh $d queue | sed -E 's/^([0-9])-[0-9]+-[0-9]+-/\1:/' | tr '\n' ' ')"
  grep -E "^[0-9:]+ ($d: |round )|cooling|launch failed|timeout|partial|aborted|INSTALL|CRASH|crash logs: [1-9]|CHAIN" $STATE/chain-$d.log 2>/dev/null | tail -5 | cut -c1-170
  tail -2 $STATE/thermal-$d.log 2>/dev/null | cut -c1-170
done
exit 0
