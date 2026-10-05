#!/bin/sh
# UIKit spring probe (LLP 1099 §3): what UIKit's spring calls really run.
# Builds a one-scene simulator app, runs it on the given simulator, prints
# its report (one line per call; formats in LLP 1099 §3) and uninstalls it.
#
#   motion/tests/fixtures/uikit-springs/probe/probe.sh <simulator udid> > report.txt
set -eu
udid=$1
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d /tmp/uikit-spring-probe.XXXXXX)
mkdir -p "$work/P.app"
cp "$here/Info.plist" "$work/P.app/Info.plist"
xcrun --sdk iphonesimulator swiftc -target arm64-apple-ios18.0-simulator "$here/main.swift" -o "$work/P.app/P"
codesign -f -s - "$work/P.app" >/dev/null 2>&1
xcrun simctl install "$udid" "$work/P.app"
out="$(xcrun simctl get_app_container "$udid" dev.exact2.springprobe data)/tmp/out.txt"
rm -f "$out"
xcrun simctl launch "$udid" dev.exact2.springprobe >/dev/null 2>&1 || true
i=0; while [ ! -s "$out" ] && [ $i -lt 600 ]; do sleep 0.5; i=$((i+1)); done
cat "$out"
xcrun simctl uninstall "$udid" dev.exact2.springprobe
rm -rf "$work"
