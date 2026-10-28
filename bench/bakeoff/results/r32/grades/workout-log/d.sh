#!/bin/zsh
app=$1; shift
case $app in A) b=dev.bakeoff.workoutlog.b9d2e18;; B) b=dev.bakeoff.workoutlog.bcc183b;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
