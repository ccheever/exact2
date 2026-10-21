#!/bin/bash
# prepare.sh — the benchmark's data, made once and copied to where each app's build reads it. All of it is ignored
# by .gitignore; gen.py is the one source.
#   data/messages.json, data/images/*.jpg   gen.py's output (deterministic; checked against data.sha256)
#   exact-heavylist/data/messages.json, exact-heavylist/assets/   the exact2 build refuses inputs outside its app directory
#   exact-heavylist/Cargo.lock               the root's, which build.mjs resolves for that workspace offline
#   expo/data/messages.json, expo/assets/images/
# The SwiftUI and UIKit builds read data/ directly. FORCE=1 regenerates data/.
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd)
if [ -n "${FORCE:-}" ] || [ ! -s "$H/data/messages.json" ] || [ "$(ls "$H/data/images" 2>/dev/null | wc -l | tr -d ' ')" != 104 ]; then
  python3 -c 'import PIL' 2>/dev/null || { echo "gen.py needs Pillow (python3 -m pip install Pillow==11.3.0)"; exit 1; }
  echo "generating data/ (about a minute)"; python3 "$H/gen.py"
fi
# The JPEG bytes depend on Pillow's encoder (11.3.0 reproduces the checksums); messages.json does not.
(cd "$H/data" && shasum -a 256 -c --quiet ../data.sha256) \
  || echo "warning: data/ differs from data.sha256; results are not comparable with runs on the published data" >&2
mkdir -p "$H/exact-heavylist/data" "$H/exact-heavylist/assets" "$H/expo/data" "$H/expo/assets/images"
rsync -a "$H/data/messages.json" "$H/exact-heavylist/data/messages.json"
rsync -a --delete "$H/data/images/" "$H/exact-heavylist/assets/"
cp "$H/../../Cargo.lock" "$H/exact-heavylist/Cargo.lock"
rsync -a "$H/data/messages.json" "$H/expo/data/messages.json"
rsync -a --delete "$H/data/images/" "$H/expo/assets/images/"
echo "prepared: data/, exact-heavylist/, expo/"
