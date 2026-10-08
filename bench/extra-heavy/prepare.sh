#!/bin/bash
# prepare.sh — every generated input of the benchmark, made from the generators here and copied to where each app's
# build reads it. All of it is ignored by .gitignore. Run it before building any app; it is quick when nothing is
# missing. FORCE=1 regenerates data/; SKIP_LOTTIE=1 skips the Lottie sources (only the SwiftUI and UIKit builds
# need them).
#   data/fonts/                   fetch-fonts.sh (google/fonts + this checkout's Inter; checked against fonts.sha256)
#   data/                         gen.py, seed 49975 (videos by gen_video.swift; checked against data.sha256)
#   exact-xheavy/assets/, exact-xheavy/icon.png, exact-xheavy/data/{feed,motion-0N}.json   exact-xheavy/gen-assets.py (the exact2 build refuses
#                                 inputs outside its app directory)
#   exact-xheavy/svgs.contract, exact-xheavy/rings.contract   exact-xheavy/gen-svgs.py, exact-xheavy/gen-rings.py
#   exact-xheavy/Cargo.lock              this checkout's Cargo.lock, copied every time: the app's shared crates stay at the
#                                 checkout's versions, and the first build resolves the app's own on top of it
#   expo/data/ (data/ without svg/), expo/assets.ts, expo/embed.ts, expo/assets/icon.png   expo/gen-assets.py
#   vendor/lottie-ios/            fetch-lottie.sh
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd)
python3 -c 'import PIL' 2>/dev/null || { echo "the generators need Pillow (python3 -m pip install Pillow==11.3.0)"; exit 1; }
"$H/fetch-fonts.sh"
if [ -n "${FORCE:-}" ] || [ ! -s "$H/data/feed.json" ] || [ "$(ls "$H/data/video" 2>/dev/null | wc -l | tr -d ' ')" != 4 ]; then
  echo "generating data/ (about a minute; the videos need Xcode's swift)"; python3 "$H/gen.py"
fi
# The JPEG/GIF/WebP bytes depend on Pillow's encoders (11.3.0 reproduces the checksums); feed.json does not. The
# videos' pixels are a pure function of the frame, their H.264 bytes are not, so they are not in the list.
(cd "$H/data" && shasum -a 256 -c --quiet ../data.sha256) \
  || echo "warning: data/ differs from data.sha256; results are not comparable with runs on the published data" >&2
python3 "$H/exact-xheavy/gen-assets.py"
python3 "$H/exact-xheavy/gen-rings.py"
python3 "$H/exact-xheavy/gen-svgs.py"
cp "$ROOT/Cargo.lock" "$H/exact-xheavy/Cargo.lock"
mkdir -p "$H/expo/data"
rsync -a --delete --exclude svg/ "$H/data/" "$H/expo/data/"
python3 "$H/expo/gen-assets.py"
[ -n "${SKIP_LOTTIE:-}" ] || "$H/fetch-lottie.sh"
echo "prepared: data/, exact-xheavy/, expo/${SKIP_LOTTIE:+ (no Lottie)}"
