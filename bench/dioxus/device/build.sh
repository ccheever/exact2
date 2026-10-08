#!/bin/bash
# build.sh — device builds of the heavy list, whole feed and bounded window, from the checkout this directory is in,
# signed with bench/heavy-list's probe:
#   whole    ../../heavy-list/exact-heavylist/build.sh   -> that directory's build/HeavyExact2.app (dev.exact.heavybench.exact)
#   bounded  host/apple/build.mjs --device --unsigned    -> build/HeavyBounded.app (dev.exact.heavybench.bounded)
# Old bundles are deleted first, any failure aborts, and each is stamped BenchBuild=heavy-<tag>@<commit>
# (+dirty when tracked files differ). Needs BENCH_SIGN_IDENTITY and BENCH_PROFILE. ONLY=whole|bounded builds one.
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/.." && pwd); ROOT=$(cd "$B/../.." && pwd); L=$B/../heavy-list
"$B/prepare.sh" >/dev/null
REV=$(git -C "$ROOT" rev-parse --short=9 HEAD)$([ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ] || echo +dirty)
if [ "${ONLY:-whole}" = whole ]; then "$L/exact-heavylist/build.sh"; fi
if [ "${ONLY:-bounded}" = bounded ]; then
  OUT=$H/build/HeavyBounded.app; rm -rf "$OUT"; mkdir -p "$H/build"
  T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
  (cd "$ROOT" && EXACT_APP_DIR=$B/exact-bounded bun host/apple/build.mjs --device exact-bounded-apple --archive "$T/app.ipa" --unsigned) \
    > "$H/build/build.log" 2>&1 || { echo "BUILD FAILED bounded at $REV (see $H/build/build.log)"; exit 1; }
  (cd "$T" && ditto -x -k app.ipa x) && mv "$T"/x/Payload/*.app "$OUT"
  echo "$REV" > "$OUT.commit"
  BENCH_BUILD=heavy-bounded@$REV "$L/probe/resign.sh" "$OUT" dev.exact.heavybench.bounded | tail -1
  echo "built heavy-bounded@$REV: $OUT"
fi
