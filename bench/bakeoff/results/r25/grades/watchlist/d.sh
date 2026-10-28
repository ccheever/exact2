#!/bin/zsh
app=$1; shift
case $app in A) b=dev.bakeoff.watchlist.b1c7e23;; B) b=dev.bakeoff.watchlist.bd05784;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
