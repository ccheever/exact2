#!/bin/bash
a=$1; shift
case $a in A) b=dev.bakeoff.notes.bcbf36f;; B) b=dev.bakeoff.notes.b477b25;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
