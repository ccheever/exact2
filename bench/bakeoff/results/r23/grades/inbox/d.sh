#!/bin/zsh
app=$1; shift
case $app in A) id=dev.bakeoff.inbox.be3c520;; B) id=dev.bakeoff.inbox.b0d7e78;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
