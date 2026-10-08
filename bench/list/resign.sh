#!/bin/bash
# resign.sh <App.app> <bundle-id> — embeds the probe (probe/build.sh's probe-ios.dylib) and a
# provisioning profile, sets the bundle id, and signs for the phone with get-task-allow, which
# DYLD_INSERT_LIBRARIES needs. BENCH_IDENTITY: a development signing identity (its SHA-1 from
# `security find-identity -v -p codesigning`); BENCH_TEAM: its team id; BENCH_PROFILE: a
# development .mobileprovision covering the phone and the bundle id (a wildcard one does).
set -e
APP=$1; BID=$2
B=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$B/../.." && pwd)
W=${BENCH_WORK:-$ROOT/target/bench/list}
ID=${BENCH_IDENTITY:?set BENCH_IDENTITY}; TEAM=${BENCH_TEAM:?set BENCH_TEAM}; PROF=${BENCH_PROFILE:?set BENCH_PROFILE}
ENT=$(mktemp -t listbench-ent); trap 'rm -f "$ENT"' EXIT
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BID" "$APP/Info.plist"
mkdir -p "$APP/Frameworks"
cp "$W/probe-ios.dylib" "$APP/Frameworks/probe.dylib"
cp "$PROF" "$APP/embedded.mobileprovision"
cat > "$ENT" <<P
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>application-identifier</key><string>$TEAM.$BID</string>
<key>com.apple.developer.team-identifier</key><string>$TEAM</string>
<key>get-task-allow</key><true/>
</dict></plist>
P
find "$APP" -name '_CodeSignature' -type d -prune -exec rm -rf {} + 2>/dev/null || true
for f in "$APP"/Frameworks/*; do
  # An exact2 GPU module is bound to the app by its signed bytes' digest (compat.embedded.gpu):
  # build.mjs already signed it with this team; re-signing would change the digest.
  case "$(basename "$f")" in libexact_gpu*.dylib)
    codesign -dv "$f" 2>&1 | grep -q "TeamIdentifier=$TEAM" && continue ;;
  esac
  codesign -f -s "$ID" --timestamp=none "$f"
done
codesign -f -s "$ID" --timestamp=none --entitlements "$ENT" "$APP"
codesign --verify --strict "$APP" && echo signed
