# Crypto list — UIKit

`App.swift` is the whole app (bundle `dev.exact.cryptobench.uikit`, "Crypto UIKit"). `./build.sh` runs
`../prepare.sh`, then builds `build-sim/CryptoUIKit.app` (simulator, ad-hoc) and `build/CryptoUIKit.app` (iphoneos,
unsigned), both with `../data/coins.json` at the bundle root; the same Info.plist keys as `../swiftui/build.sh`
plus a scene manifest. `../shoot.sh uikit <name> <delay> [ENV=VAL…]` relaunches it on `BENCH_SIM` and takes a
screenshot.

How each part is built, and why it is the same work as the other apps: `../SPEC.md`, Decisions → UIKit.

Parity (simulator, `BENCH_FREEZE=1`, top and `BENCH_START_INDEX=500`, against the SwiftUI app on the same
simulator): the same rows, glyphs, charts, pulses and pills; icon, name block and chart pixels match; the price
block differs by sub-pixel placement (< 1 pt: whole-point line heights here, fractional in SwiftUI).

First device check (iPad Pro M1, 2026-09-29, one run each): fling 119.9 fps at every speed, 0 late, busy 2.46
ms/frame, CPU 308 ms/s, peak 40 MB; live fling 119.9, 0 late; rest 119.9 fps, 21 ms/s CPU. The build before the
focus fix (SPEC, Decisions → UIKit → Focus) had about 35 late frames a second. On the simulator (60 Hz): fling
60.0 fps at every speed, 0 blank frames; rest 9 ms/s CPU. `../../heavy-list/probe/tptrace.sh
dev.exact.cryptobench.uikit fling <outdir>` records a Time Profiler trace around one run.
