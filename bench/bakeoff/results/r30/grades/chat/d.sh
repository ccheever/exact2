#!/bin/bash
app=$1; shift
case $app in A) b=dev.bakeoff.chat.bf5f63b;; B) b=dev.bakeoff.chat.b77d1bd;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
