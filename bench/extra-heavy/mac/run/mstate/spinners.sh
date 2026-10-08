#!/bin/bash
# spinners.sh start <N> | stop — N processes spinning on this Mac (a fixed integer loop, default QoS), the
# controlled scarce-CPU state for one measurement (PROGRAM.md loss 12, 2026-09-30: what a throttled or Low Power
# Mode laptop would show). PIDs go to ~/xhm/tmp/spinners.pid; stop kills exactly those. Run with STATE_GATE=off.
X=${BENCH_STAGE:-$HOME/xhm}; F=$X/tmp/spinners.pid
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
case "$1" in
  start)
    N=${2:?N}; : > $F
    for i in $(seq 1 $N); do
      $PY -c 'x=0
while True:
    for i in range(100000): x = (x * 1103515245 + 12345) & 0x7fffffff' &
      echo $! >> $F
    done
    sleep 1; echo "spinning: $(tr '\n' ' ' < $F)"; $PY $X/tmp/speed.py ;;
  stop)
    [ -f $F ] || { echo "no spinners"; exit 0; }
    for p in $(cat $F); do kill $p 2>/dev/null; done; sleep 0.5
    for p in $(cat $F); do kill -9 $p 2>/dev/null; done
    rm -f $F; echo stopped; $PY $X/tmp/speed.py ;;
  *) echo "spinners.sh start <N> | stop"; exit 2 ;;
esac
