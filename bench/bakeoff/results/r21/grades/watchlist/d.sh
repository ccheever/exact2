#!/bin/zsh
a=$1; shift
[[ $a == A ]] && b=dev.bakeoff.watchlist.02e401 || b=dev.bakeoff.watchlist.a4ae81
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
