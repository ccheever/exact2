#!/bin/bash
# resign-all.sh <app.app>… — re-sign every bundle the board installs with the current
# bench/heavy-list/probe/probe-ios.dylib (after a probe rebuild), keeping each bundle's id and BenchBuild stamp
# (unstamped ones stay unstamped). Each bundle is re-signed as a copy and renamed in, so an install never meets a
# half-signed bundle. Prints the probe sha and, per bundle, id, stamp and whether the embedded probe has the thermal
# symbol (probe sha1 6b10a15abb37 and later record the thermal state). Signing: resign.sh's BENCH_SIGN_IDENTITY,
# BENCH_PROFILE.
ROOT=$(cd "$(dirname "$0")/../.." && pwd); P=$ROOT/bench/heavy-list/probe; R=$P/resign.sh; PB=/usr/libexec/PlistBuddy
echo "probe $(shasum $P/probe-ios.dylib | cut -c1-12)"
for a in "$@"; do
  a=${a%/}
  [ -d "$a" ] || { echo "MISSING $a"; continue; }
  id=$($PB -c 'Print :CFBundleIdentifier' "$a/Info.plist"); stamp=$($PB -c 'Print :BenchBuild' "$a/Info.plist" 2>/dev/null)
  rm -rf "$a.new"; cp -R "$a" "$a.new"
  if [ -n "$stamp" ]; then BENCH_BUILD="$stamp" $R "$a.new" "$id" >/dev/null 2>&1; else $R "$a.new" "$id" >/dev/null 2>&1; fi
  codesign --verify --strict "$a.new" 2>/dev/null || { echo "SIGN FAILED $a (left as it was)"; rm -rf "$a.new"; continue; }
  rm -rf "$a.old"; mv "$a" "$a.old" && mv "$a.new" "$a" && rm -rf "$a.old"
  echo "ok $id | ${stamp:-unstamped} | thermal symbol: $(nm -u "$a/Frameworks/probe.dylib" | grep -c NSProcessInfoThermalStateDidChangeNotification) | ${a#$ROOT/}"
done
