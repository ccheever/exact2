#!/bin/bash
# build.sh — the macOS side: the measuring tools beside this script (macbench, wins, tcc; swiftc -O), then the three
# apps, each built from this checkout:
#   exact    ../../heavy-list/exact-heavylist   host/apple/build.mjs exact-heavylist-apple (ad-hoc signed)
#   bounded  ../exact-bounded                   host/apple/build.mjs exact-bounded-apple
#   dioxus   ../heavy-dx                        dx build --macos --renderer native --release --features vello
# PART=tools|exact|bounded|dioxus builds one. apps.sh finds what they produced. DX as in ../web/build.sh. Logs go to
# <checkout>/target/bench/dioxus/build-<part>.log. Run ../prepare.sh first.
set -e
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd)
L=$ROOT/target/bench/dioxus; mkdir -p "$L"
case ${PART:-all} in all|tools)
  for t in macbench wins tcc; do xcrun swiftc -O "$H/$t.swift" -o "$H/$t"; done; echo "tools built" ;;
esac
app() { # <part> <app dir> <crate>
  (cd "$ROOT" && EXACT_APP_DIR=$2 EXACT_IDENTITY=- bun host/apple/build.mjs $3) > "$L/build-$1.log" 2>&1 \
    || { tail -20 "$L/build-$1.log"; echo "$1: BUILD FAILED"; exit 1; }
  echo "$1 built at $(git -C "$ROOT" rev-parse --short=9 HEAD)"
}
case ${PART:-all} in all|exact) app exact "$B/../heavy-list/exact-heavylist" exact-heavylist-apple ;; esac
case ${PART:-all} in all|bounded) app bounded "$B/exact-bounded" exact-bounded-apple ;; esac
case ${PART:-all} in all|dioxus)
  DX=${DX:-$ROOT/target/bench/dioxus/tools/bin/dx}
  [ -x "$DX" ] || { echo "no dx at $DX: cargo install dioxus-cli --version 0.8.0-alpha.1 --locked --root $ROOT/target/bench/dioxus/tools"; exit 1; }
  # The default renderer (vello-hybrid) panics after ~9,000 pt of scrolling (README); `vello` is the full Vello one.
  (cd "$B/heavy-dx" && "$DX" build --macos --renderer native --release --features vello) > "$L/build-dioxus.log" 2>&1 \
    || { tail -20 "$L/build-dioxus.log"; echo "dioxus: BUILD FAILED"; exit 1; }
  echo "dioxus built ($("$DX" --version))" ;;
esac
