#!/bin/bash
# prepare.sh — the benchmark's data, made once and copied to where each app's build reads it. All of it is ignored
# by .gitignore; gen.py is the one source.
#   data/coins.json                           gen.py's output (seed 49975, deterministic; checked against data.sha256)
#   exact-crypto-svg/data/coins.json, exact-crypto-gpu/data/coins.json
#                                             the exact2 build refuses inputs outside its app directory
#   exact-crypto-svg/Cargo.lock, exact-crypto-gpu/Cargo.lock
#                                             the root's, which build.mjs resolves for each app's workspace
#   expo/data/coins.json                      Metro inlines it into the Hermes bundle
# The SwiftUI and UIKit builds read data/ directly. FORCE=1 regenerates data/.
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd)
if [ -n "${FORCE:-}" ] || [ ! -s "$H/data/coins.json" ]; then
  echo "generating data/"; python3 "$H/gen.py"
fi
(cd "$H/data" && shasum -a 256 -c --quiet ../data.sha256) \
  || echo "warning: data/ differs from data.sha256; results are not comparable with runs on the published data" >&2
for a in exact-crypto-svg exact-crypto-gpu; do
  mkdir -p "$H/$a/data"
  rsync -a "$H/data/coins.json" "$H/$a/data/coins.json"
  cp "$H/../../Cargo.lock" "$H/$a/Cargo.lock"
done
mkdir -p "$H/expo/data"
rsync -a "$H/data/coins.json" "$H/expo/data/coins.json"
echo "prepared: data/, exact-crypto-svg/, exact-crypto-gpu/, expo/"
