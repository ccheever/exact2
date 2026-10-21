#!/bin/bash
# build.sh — builds both web apps into dist/ beside this script: dist/exact (exact2's web build of
# ../exact-xheavy, host/web/build.mjs) and dist/expo (`npx expo export -p web` of ../expo, after npm ci there).
# PART=exact|expo builds one. WEB_FLAGS passes flags to host/web/build.mjs: the published web numbers are the
# wasm target of 2026-09-28 (`WEB_FLAGS=--wasm`); the default today is the JS target (LLP 1071), unmeasured here.
# Run ../prepare.sh first. Logs: dist/build-<part>.log.
set -e
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd)
mkdir -p "$H/dist"
if [ -z "$PART" ] || [ "$PART" = exact ]; then
  (cd "$ROOT" && EXACT_APP_DIR=$B/exact-xheavy EXACT_WEB_DIST=$H/dist/exact.new bun host/web/build.mjs exact-xheavy-web $WEB_FLAGS > "$H/dist/build-exact.log" 2>&1) \
    || { grep -n "^error" -A8 "$H/dist/build-exact.log" | head -30; tail -5 "$H/dist/build-exact.log"; exit 1; }
  rm -rf "$H/dist/exact" && mv "$H/dist/exact.new" "$H/dist/exact"
  echo "exact2 web: $(du -sh "$H/dist/exact" | cut -f1)"
fi
if [ -z "$PART" ] || [ "$PART" = expo ]; then
  (cd "$B/expo" && { [ -d node_modules ] || npm ci; } && rm -rf dist && npx expo export -p web) > "$H/dist/build-expo.log" 2>&1 \
    || { tail -30 "$H/dist/build-expo.log"; exit 1; }
  rm -rf "$H/dist/expo" && cp -R "$B/expo/dist" "$H/dist/expo"
  echo "expo web: $(du -sh "$H/dist/expo" | cut -f1)"
fi
