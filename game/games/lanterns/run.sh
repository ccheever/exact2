#!/bin/sh
set -eu
export EXACT_UPDATE_TRUST=development
APP_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export EXACT_APP_DIR="$APP_DIR"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$APP_DIR/../../target}"
export EXACT_WEB_DIST="$APP_DIR/dist"
cd "$APP_DIR/../../.."
case "${1:-proof}" in
  proof) exec bun "$APP_DIR/proof.mjs" web ;;
  build-web) exec bun "$APP_DIR/build-web.mjs" ;;
  test) bun game/app/shells.mjs "$APP_DIR"; exec cargo test --manifest-path "$APP_DIR/.shells/Cargo.toml" --locked --offline --workspace ;;
  dev) exec bun game/dev.mjs lanterns ;;
  macos) exec bun host/apple/build.mjs --run ;;
  ios) exec bun host/apple/build.mjs --ios --run ;;
  *) echo 'Usage: sh game/games/lanterns/run.sh [proof|test|build-web|dev|macos|ios]' >&2; exit 2 ;;
esac
