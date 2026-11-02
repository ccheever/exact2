#!/bin/bash
# install.sh <udid> <app.app>… — install each; exit 1 on the first failure (never measure what was there
# before). Logs each app's BenchBuild stamp and executable UUID. Caller holds the device. Install errors are
# appended to $BENCH_RESULTS/install-errors.log (default <checkout>/target/bench/extra-heavy/results).
U=${1:?udid}; shift
E=${BENCH_RESULTS:-$(cd "$(dirname "$0")/../../.." && pwd)/target/bench/extra-heavy/results}/install-errors.log; mkdir -p "$(dirname "$E")"
for a in "$@"; do
  out=$(timeout 180 xcrun devicectl device install app --device $U "$a" 2>&1); rc=$?
  # a hung install (the cap, exit 124, three times on the iPad on 2026-09-30) is tried once more before failing
  [ $rc = 124 ] && { echo "install of $a hung (180 s); trying once more"; out=$(timeout 180 xcrun devicectl device install app --device $U "$a" 2>&1); rc=$?; }
  # installd on the iPad refused the same bundle's signature intermittently on 2026-09-30 (18:47, 20:12: "integrity
  # could not be verified", 0xe8008001) while installing it fine before and after: one more try, the first error kept
  echo "$out" | grep -q "App installed" || { echo "install of $a refused: $(echo "$out" | grep -m1 -oE 'CoreDeviceError error [0-9]+|verify code signature[^:]*' ); trying once more"
    { echo "== $(date '+%F %T') $U $a (first try, exit $rc)"; echo "$out" | tail -8; } >> "$E"
    sleep 5; out=$(timeout 180 xcrun devicectl device install app --device $U "$a" 2>&1); rc=$?; }
  echo "$out" | grep -q "App installed" || { echo "INSTALL FAILED $a"; { echo "== $(date '+%F %T') $U $a (exit $rc)"; echo "$out" | tail -15; } >> "$E"; exit 1; }
  echo "installed $a: $(/usr/libexec/PlistBuddy -c 'Print :BenchBuild' "$a/Info.plist" 2>/dev/null || echo unstamped) uuid $(dwarfdump --uuid "$a/$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$a/Info.plist")" 2>/dev/null | head -1 | cut -d' ' -f2)"
done
