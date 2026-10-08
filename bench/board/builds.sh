#!/bin/bash
# builds.sh <tag> [commit] — the board's exact2 builds, one at a time (the Mac is shared), at this checkout's HEAD:
#   xheavy  bench/extra-heavy/ios/build-exact.sh <tag> -> bench/extra-heavy/device/exact2-<tag>/app/Payload/<App>.app
#           (dev.exact.xheavy.exact2, stamped xheavy@<commit>)
#   heavy   bench/heavy-list/exact-heavylist/build.sh, copied to $STATE/device/HeavyExact2-<tag>.app
#           (dev.exact.heavybench.exact, stamped heavy@<commit>)
#   crypto  bench/crypto-list/build-exact.sh svg, copied to $STATE/device/CryptoExact2SVGI-<tag>.app
#           (dev.exact.cryptobench.svgi, stamped crypto-svgi@<commit>)
# Each leaves $STATE/built-<what>-<tag> or $STATE/failed-<what>-<tag> for chain.sh, which waits on them (STATE is
# BENCH_STATE, default <checkout>/target/bench/board). ONLY="crypto heavy" builds a subset.
# The device-run rules: the old bundle is deleted first; any failure aborts that build; the commit is recorded beside
# the bundle and stamped into it (BenchBuild, which the probe writes into every result); nothing is installed —
# installs happen under the device lock. A dirty checkout (tracked files) is refused; with [commit], so is a HEAD
# that is not that commit. EXACT2=<another exact2 checkout> builds xheavy's exact2 from it (another commit in the same
# rotation: chain.sh TIP_TAG); heavy and crypto always build at this checkout.
# Signing: BENCH_SIGN_IDENTITY, BENCH_PROFILE (BENCH_TEAM), as bench/heavy-list/probe/resign.sh reads them.
BD=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$BD/../.." && pwd)
TAG=${1:?tag}; WANT=${2:-}; STATE=${BENCH_STATE:-$ROOT/target/bench/board}; DV=$STATE/device; mkdir -p $DV
W=${EXACT2:-$ROOT}
if [ -n "$WANT" ] && [ "$(git -C "$W" rev-parse HEAD)" != "$(git -C "$W" rev-parse "$WANT^{commit}" 2>/dev/null)" ]; then
  echo "refusing: $W is at $(git -C "$W" rev-parse --short=9 HEAD), not $WANT"; exit 1
fi
[ -z "$(git -C "$W" status --porcelain --untracked-files=no)" ] || { echo "refusing: $W has uncommitted changes to tracked files"; exit 1; }
REV=$(git -C "$W" rev-parse --short=9 HEAD)
mark() { rm -f $STATE/built-$1-$TAG $STATE/failed-$1-$TAG; if "$2"; then touch $STATE/built-$1-$TAG; else touch $STATE/failed-$1-$TAG; fi; }
b_xheavy() { "$ROOT/bench/extra-heavy/ios/build-exact.sh" $TAG; }
b_heavy() {
  [ -z "$EXACT2" ] || { echo "heavy builds at this checkout only (EXACT2 is set)"; return 1; }
  local O=$DV/HeavyExact2-$TAG.app H=$ROOT/bench/heavy-list/exact-heavylist/build
  rm -rf "$O" "$O.commit"
  "$ROOT/bench/heavy-list/exact-heavylist/build.sh" || return 1
  ditto "$H/HeavyExact2.app" "$O" && cp "$H/HeavyExact2.app.commit" "$O.commit" && echo "board copy: $O"
}
b_crypto() {
  [ -z "$EXACT2" ] || { echo "crypto builds at this checkout only (EXACT2 is set)"; return 1; }
  local O=$DV/CryptoExact2SVGI-$TAG.app C=$ROOT/bench/crypto-list/exact-crypto-svg/build
  rm -rf "$O" "$O.commit"
  "$ROOT/bench/crypto-list/build-exact.sh" svg || return 1
  ditto "$C/CryptoExact2SVGI.app" "$O" && cp "$C/CryptoExact2SVGI.app.commit" "$O.commit" && echo "board copy: $O"
}
for what in ${ONLY:-xheavy crypto heavy}; do
  echo "$(date +%T) building $what at $REV (tag $TAG)"
  mark $what b_$what
  [ -e $STATE/built-$what-$TAG ] && echo "$(date +%T) $what built" || echo "$(date +%T) $what FAILED"
done
echo "builds finished $(date +%T)"
