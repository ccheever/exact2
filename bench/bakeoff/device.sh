#!/bin/bash
# device.sh — a bake-off app on a real iPhone or iPad, measured by the scroll probe (DEVICE.md).
#
#   device.sh build <app-dir> <out-dir> [--iphone]   build one app for the device into <out-dir>/<name>.app
#   device.sh run <out-dir> <name>…                   sign each with the probe (bundle id dev.bakeoff.device.<name>),
#                                                     install it, run the probe's scenarios, uninstall it
#
# <app-dir> is a round's app (~/bakeoff/rounds/<round>/apps/<spec>-<arm>). A native app (build.sh, Sources/) is
# compiled as the skeleton's build.sh does, for iphoneos, -O with whole-module optimization. An Exact app (app.json)
# is copied to <out-dir>/src/<name>, pointed at this checkout (`exact.mjs update`) and built by host/apple/build.mjs
# as an unsigned release archive, as it ships. Both declare iPhone and iPad, so on an iPad both run full screen;
# --iphone declares iPhone only (compatibility mode on an iPad).
#
# run's environment: BENCH_DEVICE (udid), BENCH_SIGN_IDENTITY, BENCH_PROFILE (a profile covering
# dev.bakeoff.device.*), as ../heavy-list/probe/resign.sh and devrun.sh, and SCENARIOS (default "fling ladder"), RUNS (default 2),
# BENCH_DELAY (default 8). Exact builds share CARGO_TARGET_DIR (default <out-dir>/target).
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd); P=$ROOT/bench/heavy-list/probe

build() {
  local SRC=${1%/} OUT=$2 FAMILY='<integer>1</integer><integer>2</integer>'
  [ "${3:-}" = --iphone ] && FAMILY='<integer>1</integer>'
  local NAME; NAME=$(basename "$SRC"); mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
  local APP=$OUT/$NAME.app; rm -rf "$APP"
  [ -s "$P/probe-ios.dylib" ] || "$P/build.sh" >/dev/null
  if [ -f "$SRC/app.json" ]; then
    local W=$OUT/src/$NAME
    mkdir -p "$OUT/src"; rsync -a --delete --exclude target --exclude build --exclude .exact "$SRC/" "$W/"
    local CRATE; CRATE=$(sed -n 's/^name = "\(.*\)"/\1/p' "$W/apple/Cargo.toml" | head -1)
    # iPhone only: the manifest's device family, before the build reads it.
    # The manifest, before the build reads it: iPhone only with --iphone, and an icon when it names none (a
    # distribution archive requires one; a round's builders test on a simulator, which does not).
    python3 - "$W/app.json" "${3:-}" "$ROOT/apps/spark/assets/icon.png" <<'PY'
import json, os, shutil, sys
path, flag, stock = sys.argv[1:]
m = json.load(open(path))
if flag == '--iphone': m['host']['ios']['deviceFamily'] = ['iphone']
if not m.get('icons') and not m.get('host', {}).get('ios', {}).get('icon'):
    os.makedirs(os.path.join(os.path.dirname(path), 'assets'), exist_ok=True)
    shutil.copy(stock, os.path.join(os.path.dirname(path), 'assets', 'device-icon.png'))
    m['icons'] = [{'src': 'assets/device-icon.png', 'sizes': '1024x1024', 'type': 'image/png'}]
json.dump(m, open(path, 'w'), indent=2)
PY
    (cd "$W" && EXACT2=$ROOT bun exact.mjs update >/dev/null && EXACT2=$ROOT bun exact.mjs lock >/dev/null)
    local T; T=$(mktemp -d)
    (cd "$ROOT" && CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$OUT/target} EXACT_APP_DIR=$W \
      bun host/apple/build.mjs --device "$CRATE" --archive "$T/app.ipa" --unsigned) > "$OUT/$NAME.build.log" 2>&1 \
      || { echo "BUILD FAILED: $NAME (see $OUT/$NAME.build.log)"; rm -rf "$T"; return 1; }
    (cd "$T" && ditto -x -k app.ipa x) && mv "$T"/x/Payload/*.app "$APP"; rm -rf "$T"
  else
    # shellcheck disable=SC1091
    local APP_NAME APP_ID EXTRA_PLIST; source "$SRC/app.env"
    mkdir -p "$APP"
    [ -d "$SRC/Resources" ] && find "$SRC/Resources" -mindepth 1 -maxdepth 1 -exec cp -R {} "$APP/" \;
    cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>App</string>
<key>CFBundleIdentifier</key><string>$APP_ID</string>
<key>CFBundleName</key><string>$APP_NAME</string>
<key>CFBundleDisplayName</key><string>$APP_NAME</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleSupportedPlatforms</key><array><string>iPhoneOS</string></array>
<key>DTPlatformName</key><string>iphoneos</string>
<key>MinimumOSVersion</key><string>26.0</string>
<key>UIDeviceFamily</key><array>$FAMILY</array>
<key>UIRequiresFullScreen</key><true/>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSupportsIndirectInputEvents</key><true/>
<key>CADisableMinimumFrameDurationOnPhone</key><true/>
<key>UISupportedInterfaceOrientations</key><array><string>UIInterfaceOrientationPortrait</string></array>
${EXTRA_PLIST:-}
</dict></plist>
PLIST
    xcrun -sdk iphoneos swiftc -parse-as-library -O -whole-module-optimization -module-name App \
      -target arm64-apple-ios26.0 "$SRC"/Sources/*.swift -o "$APP/App" > "$OUT/$NAME.build.log" 2>&1 \
      || { echo "BUILD FAILED: $NAME (see $OUT/$NAME.build.log)"; return 1; }
  fi
  echo "$NAME@$(git -C "$ROOT" rev-parse --short=9 HEAD)" > "$APP.stamp"
  echo "built $APP"
}

run() {
  local OUT; OUT=$(cd "$1" && pwd); shift
  : "${BENCH_DEVICE:?set BENCH_DEVICE to the device udid}"
  export BENCH_DELAY=${BENCH_DELAY:-8}
  local R=$OUT/results NAME S i; mkdir -p "$R"
  # Bounce: a bake-off list (12,000 to 60,000 pt) is shorter than a fast segment's travel.
  local ENVS="BENCH_BOUNCE=1,BENCH_MINHEIGHT=4000${BENCH_ENV:+,$BENCH_ENV}"
  # Everything installed is uninstalled however the run ends.
  INSTALLED="$*"
  trap 'for n in $INSTALLED; do xcrun devicectl device uninstall app --device "$BENCH_DEVICE" "dev.bakeoff.device.$n" >/dev/null 2>&1; done' EXIT
  for NAME in "$@"; do
    # Signed here, not at build: signing needs the login keychain unlocked, building does not.
    BENCH_BUILD=$(cat "$OUT/$NAME.app.stamp") "$P/resign.sh" "$OUT/$NAME.app" "dev.bakeoff.device.$NAME" | tail -1
    xcrun devicectl device install app --device "$BENCH_DEVICE" "$OUT/$NAME.app" >/dev/null
  done
  # Rounds outermost, so a device warming up spreads over every app alike.
  for i in $(seq 1 "${RUNS:-2}"); do
    for NAME in "$@"; do
      for S in ${SCENARIOS:-fling ladder}; do
        BENCH_ENV=$ENVS "$P/devrun.sh" "dev.bakeoff.device.$NAME" "$S" 0 "$R/$NAME-$S-t-$i.json" || true
      done
    done
  done
  # Blank bands, from a layer render every 4th frame (perturbs timing; its own run).
  for NAME in "$@"; do
    BENCH_RENDER=layer BENCH_ENV=$ENVS "$P/devrun.sh" "dev.bakeoff.device.$NAME" fling 4 "$R/$NAME-fling-b-1.json" || true
  done
  python3 "$P/summarize.py" "$R" "$(IFS=,; echo "$*")"
}

case ${1:-} in
  build) shift; build "$@" ;;
  run) shift; run "$@" ;;
  *) sed -n 2,18p "$0"; exit 2 ;;
esac
