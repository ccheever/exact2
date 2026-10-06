#!/bin/zsh
# usage: drive.sh <web|macos> <label>   (run from the app dir)
host=$1; label=$2; S=${0:A:h}
export PATH="$HOME/.bun-1.4.2/bin:$PATH"
ops=("clock +0")
for k in 1 2 3 4 5 6 7; do ops+=("clock +200 real" tree state); done
ops+=("screenshot $S/$label-$host-1400.png")
for k in 8 9 10; do ops+=("clock +200 real" tree state); done
ops+=("clock +1000 real" tree state "screenshot $S/$label-$host-3000.png" logs)
bun exact.mjs agent $host "${ops[@]}" > $S/$label-$host.log 2>&1
echo "exit $?"
python3 -I $S/table.py $S/$label-$host.log > $S/$label-$host.table.txt
cat $S/$label-$host.table.txt
