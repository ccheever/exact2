#!/bin/bash
app=$1; shift
case $app in A) id=dev.bakeoff.chat.b2e4fb3;; B) id=dev.bakeoff.chat.b6db089;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
