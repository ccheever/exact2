#!/bin/bash
# kindshots.sh <out-dir> [KIND …] — parity screenshots with the first row of each kind scrolled to the top
# (found by the kind label in the row header, a leaf whose text is the kind, CSS-uppercased). Uses shots.py (server.py must be running).
W=$(cd "$(dirname "$0")" && pwd); OUT=$1; shift; T=$(mktemp -d -t xhw-kind); trap 'rm -rf "$T"' EXIT
for k in ${@:-PHOTO THUMBS SHADER CANVAS SVG VIDEO MAP MARKDOWN CODE INTL TYPEFACE CAROUSEL MOTION GLASS LIVE THREAD WEBVIEW FILMSTRIP INBOX}; do
  XHW_AFTER=4 XHW_EVAL="(async () => { const want = '$k'.toLowerCase(); const s=[...document.querySelectorAll('*')].find(e => e.scrollHeight > 20000 && e.clientHeight >= 300);
    for (let i = 0; i < 80; i++) { const x = [...document.querySelectorAll('*')].find(e => !e.children.length && (e.textContent || '').trim().toLowerCase() === want && e.getBoundingClientRect().width < 120);
      if (x) { x.scrollIntoView({block: 'start'}); s.scrollTop -= 70; return 'ok ' + i; } s.scrollTop += 600; await new Promise(r => setTimeout(r, 250)); } return 'none'; })()" \
    python3 $W/shots.py $T 0 >/dev/null 2>&1
  mkdir -p $OUT; mv $T/exact2-0.png $OUT/exact2-$k.png; mv $T/expo-0.png $OUT/expo-$k.png
done
ls $OUT
