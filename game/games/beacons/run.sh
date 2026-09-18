#!/bin/sh
set -eu
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
export EXACT_UPDATE_TRUST=development
APP_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export EXACT_APP_DIR="$APP_DIR"
export CARGO_TARGET_DIR="$APP_DIR/target"
export EXACT_WEB_DIST="$APP_DIR/dist"
cd "$APP_DIR/../../.."
case "${1:-proof}" in
  proof) exec bun "$APP_DIR/proof.mjs" web ;;
  test) exec cargo test --manifest-path game/Cargo.toml --locked --offline -p beacons-logic ;;
  dev) exec bun game/dev.mjs beacons ;;
  *) echo 'Usage: sh game/games/beacons/run.sh [proof|test|dev]' >&2; exit 2 ;;
esac
