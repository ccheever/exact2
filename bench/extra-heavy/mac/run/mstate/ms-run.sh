#!/bin/bash
# ms-run.sh <app> <out-prefix> — one fling run on this Mac with the machine's state beside it: CPU speed
# (speed.py) and thermal pressure level just before and just after, the level and load each second during.
# Writes <prefix>.json (the probe's) and <prefix>.state. From ~/xhm.
X=${BENCH_STAGE:-$HOME/xhm}; APP=$1; P=$2
PY=/Library/Developer/CommandLineTools/usr/bin/python3; [ -x $PY ] || PY=python3
lvl() { notifyutil -g com.apple.system.thermalpressurelevel 2>/dev/null | awk '{print $2}'; }
{ echo "app $APP $(date +%T)"; echo "before $($PY $X/tmp/speed.py) thermal $(lvl) $(uptime | sed 's/.*load averages: /load /')"; } > $P.state
( while true; do echo "t $(date +%T) thermal $(lvl) $(sysctl -n vm.loadavg)"; sleep 1; done >> $P.during ) & SAMP=$!
BENCH_DELAY=15 $X/run.sh $APP fling 0 $P.json
kill $SAMP 2>/dev/null
{ echo "after $($PY $X/tmp/speed.py) thermal $(lvl) $(uptime | sed 's/.*load averages: /load /')"; echo "during-levels $(awk '{print $4}' $P.during | sort | uniq -c | tr '\n' ' ')"; } >> $P.state
rm -f $P.during
cat $P.state
