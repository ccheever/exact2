#!/bin/bash
# fetch-lottie.sh — airbnb/lottie-ios at the commit the SwiftUI and UIKit apps were measured with (2c8608c,
# 2026-09-19), into vendor/lottie-ios (Sources/, Package.swift, LICENSE: Apache 2.0). Both apps compile Sources/
# into a static Lottie module per SDK. Not committed; run once (prepare.sh does).
set -e
H=$(cd "$(dirname "$0")" && pwd); V=$H/vendor/lottie-ios
C=2c8608c6b5d6e3fc62168e321fe0a7c89aaa8693
[ "$(git -C "$V" rev-parse HEAD 2>/dev/null)" = $C ] && [ -d "$V/Sources" ] && { echo "lottie-ios $C present"; exit 0; }
rm -rf "$V"; mkdir -p "$V"
git -C "$V" init -q
git -C "$V" remote add origin https://github.com/airbnb/lottie-ios.git
git -C "$V" sparse-checkout set --no-cone /Sources/ /Package.swift /LICENSE
git -C "$V" fetch -q --depth 1 --filter=blob:none origin $C
git -C "$V" checkout -q FETCH_HEAD
echo "lottie-ios $C in $V"
