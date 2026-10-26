#!/bin/zsh
app=$1; shift
case $app in A) id=dev.bakeoff.notes.b884ecb;; B) id=dev.bakeoff.notes.b197965;; *) echo bad; exit 2;; esac
cd /Users/ccheever/bakeoff/rounds/r22/grades/notes
for i in $(seq 1 20); do
  out=$(bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@" 2>&1); rc=$?
  if echo "$out" | grep -q "touches are held by"; then sleep 3; continue; fi
  echo "$out"; exit $rc
done
echo gave up
