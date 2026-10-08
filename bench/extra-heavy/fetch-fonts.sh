#!/bin/bash
# fetch-fonts.sh — the bundled open-license fonts into data/fonts/ (an input to gen.py and every app), each
# with its licence. google/fonts main as of 2026-09-27; Inter Regular is the copy in this checkout's
# host/linux/tests/fonts. The SHA-256 of every .ttf is checked against fonts.sha256 (the set the published
# numbers were measured with): a font google/fonts has changed since is reported, not silently used.
set -e
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd)
mkdir -p "$H/data/fonts"; cd "$H/data/fonts"
G=https://github.com/google/fonts/raw/main
for p in ofl/bebasneue/BebasNeue-Regular.ttf ofl/pacifico/Pacifico-Regular.ttf ofl/dmserifdisplay/DMSerifDisplay-Regular.ttf \
  ofl/abrilfatface/AbrilFatface-Regular.ttf ofl/spacemono/SpaceMono-Regular.ttf ofl/spacemono/SpaceMono-Bold.ttf \
  ofl/lobster/Lobster-Regular.ttf apache/specialelite/SpecialElite-Regular.ttf apache/permanentmarker/PermanentMarker-Regular.ttf \
  ofl/crimsontext/CrimsonText-Regular.ttf ofl/crimsontext/CrimsonText-Italic.ttf; do
  f=$(basename $p); [ -s $f ] || curl -sfL -o $f $G/$p
  d=$(dirname $p); lic=$( [ ${p%%/*} = apache ] && echo LICENSE.txt || echo OFL.txt )
  n=$(basename $d); [ -s LICENSE-$n.txt ] || curl -sfL -o LICENSE-$n.txt $G/$d/$lic
done
[ -s Inter-Regular.ttf ] || cp "$ROOT/host/linux/tests/fonts/Inter-Regular.ttf" .
[ -s LICENSE-inter.txt ] || curl -sfL -o LICENSE-inter.txt $G/ofl/inter/OFL.txt
shasum -a 256 *.ttf > SHA256SUMS
if cmp -s SHA256SUMS "$H/fonts.sha256"; then echo "fonts: 12 files, as measured"
else echo "fonts: differ from fonts.sha256 (the measured set):"; diff "$H/fonts.sha256" SHA256SUMS || true; fi
