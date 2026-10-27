#!/bin/zsh
w=$1; shift
[[ $w == A ]] && id=dev.bakeoff.photos.b3173bc || id=dev.bakeoff.photos.b72a95b
exec bun /Users/ccheever/projects/exact2-bakeoff/bench/bakeoff/drive.mjs --udid 82D8930B-3C17-48B9-8264-4B173F6DF361 --app $id "$@"
