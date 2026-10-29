#!/bin/zsh
a=$1; shift
case $a in A) b=dev.bakeoff.chat.bb6e214;; B) b=dev.bakeoff.chat.bfe00bc;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $b "$@"
