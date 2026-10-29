#!/bin/zsh
app=$1; shift
[[ $app == A ]] && app=dev.bakeoff.timetable.ba28f73
[[ $app == B ]] && app=dev.bakeoff.timetable.bd0113a
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $app "$@"
