#!/bin/zsh
app=$1; shift
case $app in A) id=dev.bakeoff.moodboard.b878f6d;; B) id=dev.bakeoff.moodboard.b45fcc3;; esac
cd /Users/ccheever/bakeoff/rounds/r32/grades/moodboard
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
