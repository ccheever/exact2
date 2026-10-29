#!/bin/bash
w=$1; shift
case $w in A) id=dev.bakeoff.moodboard.b5190b8;; B) id=dev.bakeoff.moodboard.b106018;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $id "$@"
