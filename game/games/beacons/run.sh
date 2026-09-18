#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export EXACT_UPDATE_TRUST=development
export CARGO_TARGET_DIR="$PWD/game/games/beacons/target"
export EXACT_APP_DIR="$PWD/game/games/beacons"
export EXACT_WEB_DIST="$EXACT_APP_DIR/dist"
case "${1:-proof}" in
  test) cargo test --locked --offline --manifest-path game/Cargo.toml -p beacons-logic -- --nocapture ;;
  dev) bun game/dev.mjs beacons ;;
  proof) bun game/games/beacons/proof.mjs web ;;
  *) echo 'Usage: run.sh [test|dev|proof]' >&2; exit 1 ;;
esac
