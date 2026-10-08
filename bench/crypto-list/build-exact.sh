#!/bin/bash
# build-exact.sh <svg|gpu> — builds an exact2 app for a device from the checkout this directory is in: an archive from
# host/apple/build.mjs (EXACT_APP_DIR names the app's directory; unsigned for svg, signed with BENCH_SIGN_IDENTITY and
# BENCH_PROFILE for gpu, whose GPU module's digest is baked into the app and must not change), unpacked to
# exact-crypto-<v>/build/CryptoExact2<V>.app, signed with the probe by ../heavy-list/probe/resign.sh as
# dev.exact.cryptobench.<svgi|gpu> and stamped BenchBuild=crypto-<svgi|gpu>@<commit> (+dirty when tracked files differ
# from it). The old bundle is deleted first and any failure aborts, so a series never measures a stale build.
# The simulator build is build.mjs --ios (README.md). Needs BENCH_SIGN_IDENTITY and BENCH_PROFILE (resign.sh).
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd)
case ${1:-} in
  svg) A=$H/exact-crypto-svg; CRATE=exact-crypto-svg-apple; APP=svgi; OUT=$A/build/CryptoExact2SVGI.app; SIGN=(--unsigned) ;;
  gpu) A=$H/exact-crypto-gpu; CRATE=exact-crypto-gpu-apple; APP=gpu; OUT=$A/build/CryptoExact2GPU.app; SIGN=()
       export EXACT_IDENTITY=${BENCH_SIGN_IDENTITY:?set BENCH_SIGN_IDENTITY} EXACT_PROFILE=${BENCH_PROFILE:?set BENCH_PROFILE} ;;
  *) echo "usage: build-exact.sh <svg|gpu>"; exit 2 ;;
esac
"$H/prepare.sh"
rm -rf "$OUT"; mkdir -p "$A/build"
REV=$(git -C "$ROOT" rev-parse --short=9 HEAD)$([ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ] || echo +dirty)
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
(cd "$ROOT" && EXACT_APP_DIR=$A bun host/apple/build.mjs --device $CRATE --archive "$T/app.ipa" ${SIGN[@]+"${SIGN[@]}"}) \
  > "$A/build/build.log" 2>&1 || { echo "BUILD FAILED at $REV (see $A/build/build.log)"; exit 1; }
[ -s "$T/app.ipa" ] || { echo "BUILD FAILED at $REV: no archive"; exit 1; }
(cd "$T" && ditto -x -k app.ipa x)
mv "$T"/x/Payload/*.app "$OUT"
echo "$REV" > "$OUT.commit"
BENCH_BUILD=crypto-$APP@$REV "$H/../heavy-list/probe/resign.sh" "$OUT" dev.exact.cryptobench.$APP | tail -1
echo "built crypto-$APP@$REV: $OUT"
