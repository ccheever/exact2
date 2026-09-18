#!/bin/sh
set -eu
cd "$(dirname "$0")/../../.."
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk
export EXACT_UPDATE_TRUST=development
export EXACT_APP_DIR="$PWD/game/games/beacons"
export CARGO_TARGET_DIR="$EXACT_APP_DIR/target"
export EXACT_WEB_DIST="$EXACT_APP_DIR/dist"
case "${1:-proof}" in
  test) cargo test --locked --offline --manifest-path game/Cargo.toml -p beacons-logic ;;
  proof) bun game/games/beacons/proof.mjs web ;;
  dev) bun game/dev.mjs beacons ;;
esac
