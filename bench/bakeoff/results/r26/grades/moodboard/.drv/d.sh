#!/bin/zsh
app=$1; shift
case $app in A) b=dev.bakeoff.moodboard.b13528b;; B) b=dev.bakeoff.moodboard.b0a106d;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $b "$@"
