#!/bin/sh
app=$1; shift
case $app in A) id=dev.bakeoff.feed.b046c92;; B) id=dev.bakeoff.feed.bfa6f63;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
