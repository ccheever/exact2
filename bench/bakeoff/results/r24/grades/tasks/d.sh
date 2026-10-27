#!/bin/zsh
app=$1; shift
case $app in A) id=dev.bakeoff.tasks.b2e1ce9;; B) id=dev.bakeoff.tasks.b6c953f;; esac
bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $id "$@"
