#!/bin/zsh
n=$1; shift
case $n in A) id=dev.bakeoff.notes.be677c5;; B) id=dev.bakeoff.notes.bc8be55;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
