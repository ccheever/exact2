#!/bin/bash
# devclean.sh — terminate leftover bench app processes on the device BENCH_DEVICE. The caller holds the device lock,
# if any. Every probe run leaves its app alive (suspended once the next one launches); about a hundred of them made
# every launch hang on an iPad, fixed only by a reboot. series.sh runs this before each app's runs.
# Launch something right after: with no app in front an iPhone auto-locks within minutes and then refuses every
# launch until its passcode is entered by hand (`xcrun devicectl device info lockState --device <udid>` must say
# passcodeRequired: false).
# Touched: the benchmarks' own apps by bundle name (Heavy*, XHeavy*, Crypto*: the exact2 bundle is named for its app,
# "Heavy list.app"), and any app whose executable is ExactIOS (exact2 builds before the bundle took the app's name).
# Nothing else.
U=${BENCH_DEVICE:?set BENCH_DEVICE to the device UDID}
n=0
for pid in $(timeout 60 xcrun devicectl device info processes --device $U 2>/dev/null \
    | grep "/containers/Bundle/Application/" \
    | grep -E "\.app/ExactIOS *$|/(XHeavy|Heavy|Crypto)[A-Za-z0-9 -]*\.app/[A-Za-z0-9 -]+ *$" \
    | awk '{print $1}' | sort -u); do
  timeout 30 xcrun devicectl device process terminate --device $U --pid $pid >/dev/null 2>&1 && n=$((n+1))
done
echo "devclean: terminated $n"
