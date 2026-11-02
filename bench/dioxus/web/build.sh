#!/bin/bash
# build.sh — the web builds into dist/ beside this script:
#   dist/exact    exact2's production JS target of the whole-feed app (../../heavy-list/exact-heavylist)
#   dist/bounded  the same for the bounded app (../exact-bounded)
#   dist/dioxus   Dioxus Web, `dx build --web --release --features web` of ../heavy-dx
# PART=exact|bounded|dioxus builds one. exact2's production web build is two steps (as delivery makes it): the web
# crate's bake without its wasm (host/web/build.mjs --bake, production trust), then the JS target over that bake's
# plan (host/web-js/build.mjs --production). DX names the dx CLI (default <checkout>/target/bench/dioxus/tools/bin/dx;
# README.md has the install). Run ../prepare.sh first. Logs: dist/build-<part>.log.
set -e
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd)
mkdir -p "$H/dist"
exact() { # <part> <app dir> <web crate>
  local P=$1 A=$2 C=$3 R=$H/dist/$1.work L=$H/dist/build-$1.log
  rm -rf "$R" "$H/dist/$P"; mkdir -p "$R"
  { (cd "$ROOT" && EXACT_APP_DIR=$A EXACT_WEB_DIST=$R/bake EXACT_UPDATE_TRUST=production EXACT_BAKE_OUTPUT=$R/bakeout \
       CARGO_TARGET_DIR=$A/target bun host/web/build.mjs $C --bake) &&
    (cd "$ROOT" && EXACT_APP_DIR=$A EXACT_UPDATE_TRUST=production bun host/web-js/build.mjs "$(basename "$A")" \
       --plan "$R/bake/app.plan" --out "$H/dist/$P" --production); } > "$L" 2>&1 \
    || { tail -20 "$L"; echo "$P: BUILD FAILED (see $L)"; exit 1; }
  rm -rf "$R"
  echo "$P web: $(du -sh "$H/dist/$P" | cut -f1) at $(git -C "$ROOT" rev-parse --short=9 HEAD)"
}
case ${PART:-all} in all|exact) exact exact "$B/../heavy-list/exact-heavylist" exact-heavylist-web ;; esac
case ${PART:-all} in all|bounded) exact bounded "$B/exact-bounded" exact-bounded-web ;; esac
case ${PART:-all} in all|dioxus)
  DX=${DX:-$ROOT/target/bench/dioxus/tools/bin/dx}
  [ -x "$DX" ] || { echo "no dx at $DX: cargo install dioxus-cli --version 0.8.0-alpha.1 --locked --root $ROOT/target/bench/dioxus/tools"; exit 1; }
  (cd "$B/heavy-dx" && "$DX" build --web --release --features web) > "$H/dist/build-dioxus.log" 2>&1 \
    || { tail -20 "$H/dist/build-dioxus.log"; echo "dioxus: BUILD FAILED"; exit 1; }
  rm -rf "$H/dist/dioxus"; cp -R "$B/heavy-dx/target/dx/heavy-dx/release/web/public" "$H/dist/dioxus"
  echo "dioxus web: $(du -sh "$H/dist/dioxus" | cut -f1) ($("$DX" --version))" ;;
esac
