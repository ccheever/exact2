#!/bin/zsh
app=$1; shift
case $app in A) b=dev.bakeoff.contacts.bdd3e9c;; B) b=dev.bakeoff.contacts.b32b57b;; esac
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $b "$@"
