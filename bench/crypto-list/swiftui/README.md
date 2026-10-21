# Crypto list — SwiftUI

`App.swift` is the whole app (bundle `dev.exact.cryptobench.swiftui`, "Crypto SwiftUI"). `./build.sh` runs
`../prepare.sh`, then builds `build-sim/CryptoBench.app` (simulator, ad-hoc signed) and `build/CryptoBench.app`
(iphoneos, unsigned), both with `../data/coins.json` at the bundle root. `../shoot.sh swiftui <name> <delay>
[ENV=VAL…]` relaunches it on `BENCH_SIM` and takes a screenshot.

- `List` + `.listStyle(.plain)` + `ForEach(coins)` over `@State [Coin]`; separators full width, tinted.
- `SparkShape: Shape` (a `Path`), `.trim(from: 0, to: drawn)`; `onAppear` animates `drawn` 0 → 1 with
  `.easeOut(duration: 0.6)` and its completion starts the pulse; `onDisappear` resets both.
- Pulse: an 18 pt disc, `.scaleEffect` 1/3 → 1 and `.opacity` 0.5 → 0 under
  `.easeOut(duration: 1.2).repeatForever(autoreverses: false)`.
- Flash: `keyframeAnimator` triggered by the coin's flash sequence (a `MoveKeyframe` to the colour, then a
  400 ms `LinearKeyframe` back with CSS `ease`'s bezier).
- Ticks: `Timer.publish(every: 0.1, on: .main, in: .common)`; env `BENCH_LIVE` / `BENCH_SCENARIO=rest` /
  `BENCH_FREEZE` / `BENCH_START_INDEX`.
