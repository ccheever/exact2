#!/bin/bash
# makecold.sh <App.app> <bundle-id> <out-dir> — a cold-start copy: the probe linked into the executable (LC_LOAD_DYLIB,
# linkprobe.py) instead of injected with DYLD_INSERT_LIBRARIES, bundle id <bundle-id>cold, signed by resign.sh.
# Aborts on any failure; deletes the old copy first; records the source bundle's executable sha256.
set -e
SRC=${1%/}; BID=$2; OUT=$3; B=$(cd "$(dirname "$0")" && pwd)
N=$(basename "$SRC"); mkdir -p "$OUT"; rm -rf "$OUT/$N"; cp -R "$SRC" "$OUT/$N"
EXE=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$OUT/$N/Info.plist")
echo "$N $BID src-exe-sha256 $(shasum -a 256 "$SRC/$EXE" | cut -c1-16) $(date +%FT%T)" >> "$OUT/PROVENANCE.txt"
python3 "$B/linkprobe.py" "$OUT/$N/$EXE"
"$B/resign.sh" "$OUT/$N" "${BID}cold"
