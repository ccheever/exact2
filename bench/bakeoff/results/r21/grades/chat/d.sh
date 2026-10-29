#!/bin/zsh
n=$1; shift
case $n in A) id=dev.bakeoff.chat.17f4bf;; B) id=dev.bakeoff.chat.07c073;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $id "$@"
