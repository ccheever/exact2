#!/bin/zsh
w=$1; shift
[[ $w == A ]] && app=dev.bakeoff.inbox.b0fcfa5 || app=dev.bakeoff.inbox.b1899a3
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 25F76B9D-7CC1-455F-9D1D-4AC6C66542CD --app $app "$@"
