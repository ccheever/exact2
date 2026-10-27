#!/bin/zsh
app=$1; shift
case $app in A) id=dev.bakeoff.workoutlog.b9a634e;; B) id=dev.bakeoff.workoutlog.b74beba;; esac
cd /Users/ccheever/bakeoff/rounds/r25/grades/workout-log
for i in {1..20}; do out=$(bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@" 2>&1); if echo "$out" | grep -q "touches are held"; then sleep 2; else echo "$out"; exit 0; fi; done; echo "$out"
