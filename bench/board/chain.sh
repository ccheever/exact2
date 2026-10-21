#!/bin/bash
# chain.sh <iphone|ipad> <tag> [rev] — the scoreboard's series on one device, in order (bench lane, 2026-09-30):
#   1. xheavy, the Extra Heavy feed, 19 kinds  -> $BENCH_TARGET/extra-heavy/results/<tag>-19-<dev>
#   2. crypto list                             -> $BENCH_TARGET/crypto-list/results/<dev>-<tag>
#   3. plain heavy list                        -> $BENCH_TARGET/heavy-list/results/<dev>-<tag>
# exact2 (builds.sh <tag>) vs SwiftUI vs UIKit, 3 interleaved rounds (list-series.sh), the app order rotating per round,
# the device lock taken per app, devclean + that app's install at every take, holds named board-<series>. Each step
# waits for its build ($STATE/built-<what>-<tag>; a failed-* marker skips it). Each results dir gets provenance.txt.
# A series is passed over up to PASSES (4) times; SKIP_DONE fills in what a stopped pass left out, so rerunning the
# chain resumes it. A series that exits 3 (the device stopped launching) stops the chain.
#   GATE=1      every take waits for the probe's thermal reading 0 or 1 and COOL (300) s after the previous give
#               (lib.sh); the default on the iPhone, off elsewhere (GATE= turns it off)
#   TIP_TAG=<t> a second exact2 xheavy build (EXACT2=<checkout> ONLY=xheavy builds.sh <t>) rides the xheavy rounds as
#               a fourth app, exact2@<t>-19-<dev>, its results in <t>-19-<dev>
#   BENCHES="xheavy crypto heavy" picks steps; XHEAVY_MAP, CRYPTO_MAP, HEAVY_MAP override the bundles installed
#               ("<app>=<bundle.app> …"); $STATE/prio-<dev> holding a digit sets the ticket priority at every take
# rev, for provenance: default the xheavy build's recorded commit.
DEV=${1:?device}; TAG=${2:?tag}
BD=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$BD/../.." && pwd)
STATE=${BENCH_STATE:-$ROOT/target/bench/board}; DV=$STATE/device; T=${BENCH_TARGET:-$ROOT/target/bench}
X=$ROOT/bench/extra-heavy; HL=$ROOT/bench/heavy-list; CL=$ROOT/bench/crypto-list
REV=${3:-$(head -1 $X/device/exact2-$TAG/commit 2>/dev/null)}
case $DEV in iphone) U=${BENCH_IPHONE:-$BENCH_DEVICE} ;; ipad) U=${BENCH_IPAD:-$BENCH_DEVICE} ;; *) U=$BENCH_DEVICE ;; esac
[ -n "$U" ] || { echo "set BENCH_IPHONE / BENCH_IPAD (or BENCH_DEVICE) to the $DEV's UDID"; exit 1; }
export SKIP_DONE=1
[ $DEV = iphone ] && export GATE=${GATE-1} BENCH_T_INFO=${BENCH_T_INFO:-150} BENCH_T_LAUNCH=${BENCH_T_LAUNCH:-150} RUN_CAP=${RUN_CAP:-600}
export GATE COOL=${COOL:-300}
stamp() { echo "$1: $(/usr/libexec/PlistBuddy -c 'Print :BenchBuild' "$2/Info.plist" 2>/dev/null || echo unstamped), uuid $(dwarfdump --uuid "$2/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$2/Info.plist")" | head -1 | cut -d' ' -f2), $2"; }
prov() { # <results dir> <name=bundle>…
  local r=$1; shift; mkdir -p $r
  { echo "series started $(date '+%F %T') on $DEV: exact2 at $REV (build tag $TAG)${GATE:+; every take gated on thermal 0 or 1, $COOL s after the previous give}"
    echo "probe: bench/heavy-list/probe/probe-ios.dylib sha1 $(shasum $HL/probe/probe-ios.dylib | cut -c1-12), inserted by devrun.sh; devclean at every take; app order rotating per round"
    for kv in "$@"; do stamp "${kv%%=*}" "${kv#*=}"; done
  } >> $r/provenance.txt
}
waitfor() {
  [ -e $STATE/built-$1-$TAG ] || [ -e $STATE/failed-$1-$TAG ] || step "waiting for builds.sh $TAG ($1)"
  while [ ! -e $STATE/built-$1-$TAG ] && [ ! -e $STATE/failed-$1-$TAG ]; do sleep 30; done; [ -e $STATE/built-$1-$TAG ]; }
step() { echo "$(date +%T) $DEV: $1"; }
passes() { # <name> <command…>: run until a pass exits 0, at most PASSES times
  local name=$1; shift
  for p in $(seq 1 ${PASSES:-4}); do
    step "$name pass $p"
    "$@"; local rc=$?
    [ $rc = 3 ] && { step "CHAIN STOPPED: the device stopped launching"; exit 3; }
    [ $rc = 0 ] && { step "$name complete (pass $p)"; return 0; }
  done
  step "$name NOT complete after ${PASSES:-4} passes"
}
for b in ${BENCHES:-xheavy crypto heavy}; do
  case $b in
    xheavy)
      waitfor xheavy || { step "xheavy SKIPPED: its build failed"; continue; }
      XE=$(ls -d $X/device/exact2-$TAG/app/Payload/*.app | head -1)
      M=${XHEAVY_MAP:-"swiftui=$X/device/XHeavy.app uikit=$X/device/XHeavyUIKit.app exact2=$XE"}; A="swiftui uikit exact2"
      prov $T/extra-heavy/results/$TAG-19-$DEV $M
      XT=; [ -n "$TIP_TAG" ] && XT=$(ls -d $X/device/exact2-$TIP_TAG/app/Payload/*.app 2>/dev/null | head -1)
      [ -n "$TIP_TAG" ] && [ -z "$XT" ] && step "no exact2 build tagged $TIP_TAG (builds.sh): xheavy runs without it"
      if [ -n "$XT" ]; then
        M="$M exact2@$TIP_TAG-19-$DEV=$XT"; A="$A exact2@$TIP_TAG-19-$DEV"
        mkdir -p $T/extra-heavy/results/$TIP_TAG-19-$DEV
        { echo "series started $(date '+%F %T') on $DEV: exact2 at $(head -1 $X/device/exact2-$TIP_TAG/commit) (build tag $TIP_TAG), in the same rotation as swiftui, uikit and exact2 at $REV ($TAG-19-$DEV)"; stamp exact2 $XT; } >> $T/extra-heavy/results/$TIP_TAG-19-$DEV/provenance.txt
      fi
      passes xheavy env SERIES=$TAG-19-$DEV APPS="$A" INSTALL_MAP="$M" $BD/list-series.sh xheavy $DEV ;;
    crypto)
      waitfor crypto || { step "crypto SKIPPED: its build failed"; continue; }
      M=${CRYPTO_MAP:-"swiftui=$CL/swiftui/build/CryptoBench.app uikit=$CL/uikit/build/CryptoUIKit.app svgi=$DV/CryptoExact2SVGI-$TAG.app"}
      prov $T/crypto-list/results/$DEV-$TAG $M
      passes crypto env SERIES=$DEV-$TAG INSTALL_MAP="$M" $BD/list-series.sh crypto $DEV ;;
    heavy)
      waitfor heavy || { step "heavy SKIPPED: its build failed"; continue; }
      M=${HEAVY_MAP:-"swiftui=$HL/swiftui/build/HeavyBench.app uikit=$HL/uikit/build/HeavyUIKit.app exact=$DV/HeavyExact2-$TAG.app"}
      prov $T/heavy-list/results/$DEV-$TAG $M
      passes heavy env SERIES=$DEV-$TAG INSTALL_MAP="$M" $BD/list-series.sh heavy $DEV ;;
    *) step "unknown bench $b" ;;
  esac
done
step "CHAIN DONE"
