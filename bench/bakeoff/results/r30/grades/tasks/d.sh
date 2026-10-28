#!/bin/sh
app=$1; shift
case $app in A) id=dev.bakeoff.tasks.bd50fcb;; B) id=dev.bakeoff.tasks.b616304;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $id "$@"
