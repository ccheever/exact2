#!/bin/zsh
# send.sh <dir> <n> : writes ops from stdin to cmd-n.txt and waits for out-n.json (max 300 s)
dir=$1; n=$2
cat > "$dir/cmd-$n.tmp" && mv "$dir/cmd-$n.tmp" "$dir/cmd-$n.txt"
for i in $(seq 1 1500); do [ -f "$dir/out-$n.json" ] && break; sleep 0.2; done
[ -f "$dir/out-$n.json" ] && python3 -c "
import json,sys
for e in json.load(open('$dir/out-$n.json')):
    r = e.get('r'); err = e.get('error')
    s = json.dumps(r)[:300] if r is not None else ''
    print(e['op'][:80], '|', e['ms'], 'ms |', ('ERROR ' + err[:400]) if err else s)
" || echo "timeout waiting for out-$n.json"
