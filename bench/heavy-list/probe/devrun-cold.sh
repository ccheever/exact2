#!/bin/bash
# devrun-cold.sh <bundle-id> <scenario> <sample> <local-out.json> — devrun.sh without DYLD_INSERT_LIBRARIES, for the
# makecold.sh copies (the probe linked into the executable, so dyld keeps the app's prebuilt launch closure).
BENCH_INSERT=0 exec "$(cd "$(dirname "$0")" && pwd)/devrun.sh" "$@"
