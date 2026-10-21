#!/bin/bash
# resign.sh <App.app> <bundle-id> — sets the bundle id, embeds this directory's probe-ios.dylib (build.sh) as
# Frameworks/probe.dylib and a provisioning profile, and signs the bundle for a device.
#   BENCH_SIGN_IDENTITY  the signing identity's SHA-1 (security find-identity -v -p codesigning)
#   BENCH_PROFILE        a development provisioning profile (.mobileprovision) that covers the bundle id and the
#                        device, e.g. a wildcard profile
#   BENCH_TEAM           the team id; read from the profile when unset
#   BENCH_BUILD=<text>   optional: stamped as Info.plist BenchBuild; the probe writes it into every result
set -e
APP=${1%/}; BID=$2
B=$(cd "$(dirname "$0")" && pwd)
ID=${BENCH_SIGN_IDENTITY:?set BENCH_SIGN_IDENTITY to a codesigning identity SHA-1}
PROF=${BENCH_PROFILE:?set BENCH_PROFILE to a .mobileprovision that covers $BID}
[ -s "$B/probe-ios.dylib" ] || { echo "no $B/probe-ios.dylib: run $B/build.sh first"; exit 1; }
TEAM=${BENCH_TEAM:-$(security cms -D -i "$PROF" 2>/dev/null | plutil -extract TeamIdentifier.0 raw -o - -)}
[ -n "$TEAM" ] || { echo "no team id: set BENCH_TEAM"; exit 1; }
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BID" "$APP/Info.plist"
if [ -n "$BENCH_BUILD" ]; then /usr/libexec/PlistBuddy -c "Delete :BenchBuild" "$APP/Info.plist" 2>/dev/null || true; /usr/libexec/PlistBuddy -c "Add :BenchBuild string $BENCH_BUILD" "$APP/Info.plist"; fi
mkdir -p "$APP/Frameworks"
cp "$B/probe-ios.dylib" "$APP/Frameworks/probe.dylib"
cp "$PROF" "$APP/embedded.mobileprovision"
ENT=$(mktemp -t resign-ent); trap 'rm -f "$ENT"' EXIT
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
  codesign -f -s $ID --timestamp=none "$f"
done
codesign -f -s $ID --timestamp=none --entitlements "$ENT" "$APP"
codesign --verify --strict "$APP" && echo signed
