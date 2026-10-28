#!/bin/zsh
# Creates an iPhone Duo simulator on iOS 27.1, runs the probe's UI tests on it, copies the JSON results into results/
# and deletes the simulator. Arguments name test methods to run (for example `testUpdateLink`); none runs them all.
set -euo pipefail

here=${0:A:h}
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode-27.1.app/Contents/Developer}
bundle=dev.exact.probe.TimerWaitsProbe
# Keep derived data out of the source tree, in the repository's ignored target/
build=${PROBE_BUILD_DIR:-${here:h:h:h}/target/timer-wait-points-ios}

udid=$(xcrun simctl create "Timer Waits Probe" "iPhone Duo" com.apple.CoreSimulator.SimRuntime.iOS-27-1)
echo "Created simulator $udid"
switcher=
cleanup() {
  if [[ -n $switcher ]]; then
    pkill -P $switcher 2>/dev/null || true
    kill $switcher 2>/dev/null || true
  fi
  xcrun simctl shutdown "$udid" 2>/dev/null || true
  xcrun simctl delete "$udid"
  echo "Deleted simulator $udid"
}
trap cleanup EXIT

xcodegen generate --quiet --spec "$here/project.yml" --project "$here"
xcrun simctl boot "$udid"
xcrun simctl bootstatus "$udid" -b >/dev/null

# Switches to Settings whenever the app posts `backgroundNow`, which it does as testScrollState's last fling starts to
# decelerate. The UI test cannot: XCUITest waits for the app to idle after a swipe, which is after the deceleration.
(
  while xcrun simctl spawn "$udid" notifyutil -1 dev.exact.timer-waits.backgroundNow >/dev/null; do
    xcrun simctl launch "$udid" com.apple.Preferences >/dev/null || true
  done
) &
switcher=$!

only=()
for test in "$@"; do
  only+=("-only-testing:TimerWaitsProbeUITests/TimerWaitsProbeUITests/$test")
done

mkdir -p "$build"
rm -rf "$build/results.xcresult"
set +e
xcodebuild test \
  -project "$here/TimerWaitsProbe.xcodeproj" \
  -scheme TimerWaitsProbe \
  -destination "id=$udid" \
  -derivedDataPath "$build" \
  -resultBundlePath "$build/results.xcresult" \
  -collect-test-diagnostics never \
  "${only[@]}" >"$build/xcodebuild.log" 2>&1
result=$?
set -e
grep -E '^(Test Case|Test Suite .* (passed|failed)|\*\* |.*: error:)' "$build/xcodebuild.log" || true

container=$(xcrun simctl get_app_container "$udid" "$bundle" data)

# The control: the updateLink script with no UI test attached, since XCUITest's in-app agent makes UI updates of its
# own. The app writes its log when the script ends.
if (( $# == 0 || ${@[(Ie)testUpdateLink]} )); then
  for repetition in 1 2 3; do
    file="$container/Documents/control-updateLink-$repetition.json"
    rm -f "$file"
    xcrun simctl launch --terminate-running-process "$udid" "$bundle" -scenario updateLink -rep $repetition -driver control >/dev/null
    # Nothing signals the end of the launch transition, which `XCUIApplication.launch` waits for, so allow 3 s
    sleep 3
    xcrun simctl notify_post "$udid" dev.exact.timer-waits.script
    for _ in {1..600}; do
      [[ -f $file ]] && break
      sleep 0.1
    done
    if [[ ! -f $file ]]; then
      echo "The control's run $repetition wrote no log within 60 s of the script signal" >&2
      result=1
    fi
  done
  xcrun simctl terminate "$udid" "$bundle" || true
  echo "Ran the control"
fi

mkdir -p "$here/results"
files=("$container"/Documents/*.json(N))
if (( ${#files} )); then
  cp "${files[@]}" "$here/results/"
fi
echo "Copied ${#files} files into $here/results"
exit $result
