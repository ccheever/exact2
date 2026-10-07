#!/bin/sh
# run.sh <exact.mjs> <workdir>: fixture, sign with exact release's order, then check.
EXACT=$1; D=$2
sh "$(dirname "$0")/fixture.sh" "$D" >/dev/null
cd "$D"
echo "## exact.mjs: $EXACT"
bun "$(dirname "$0")/sign.mjs" "$EXACT" "$D/Fixture.app"
echo; echo '$ codesign --verify --deep --strict -v Fixture.app'; codesign --verify --deep --strict -v Fixture.app 2>&1; echo "exit $?"
for f in Contents/Resources/assets/helper Contents/MacOS/spawn-helper Contents/Helpers/Inner.app; do
  echo; echo "\$ codesign -dv Fixture.app/$f"; codesign -dv Fixture.app/$f 2>&1 | grep -E 'flags=|not signed|Identifier=|Signature=' ; done
echo; echo '$ codesign --verify --strict -v Fixture.app/Contents/Resources/assets/helper'; codesign --verify --strict -v Fixture.app/Contents/Resources/assets/helper 2>&1; echo "exit $?"
