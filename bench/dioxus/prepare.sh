#!/bin/bash
# prepare.sh — the heavy list's data (bench/heavy-list/prepare.sh makes it and fills that bench's exact2 app, which
# this one measures as the whole-feed exact2 app), copied to where this bench's other builds read it. All of it is
# ignored by .gitignore.
#   heavy-dx/src/messages.json, heavy-dx/assets/   Dioxus's `include_str!` and `asset!` take paths inside the crate
#   exact-bounded/data/messages.json, exact-bounded/assets/   the exact2 build refuses inputs outside its app directory
#   exact-bounded/Cargo.lock                       the root's, which build.mjs resolves for that workspace
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); L=$H/../heavy-list
"$L/prepare.sh"
mkdir -p "$H/heavy-dx/assets" "$H/exact-bounded/data" "$H/exact-bounded/assets"
rsync -a "$L/data/messages.json" "$H/heavy-dx/src/messages.json"
rsync -a --delete "$L/data/images/" "$H/heavy-dx/assets/"
rsync -a "$L/data/messages.json" "$H/exact-bounded/data/messages.json"
rsync -a --delete "$L/data/images/" "$H/exact-bounded/assets/"
cp "$H/../../Cargo.lock" "$H/exact-bounded/Cargo.lock"
echo "prepared: heavy-dx/, exact-bounded/ (and bench/heavy-list)"
