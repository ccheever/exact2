#!/bin/bash
app=$1; shift
case $app in A) b=dev.bakeoff.chat.b1fd19a;; B) b=dev.bakeoff.chat.bb90e69;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
