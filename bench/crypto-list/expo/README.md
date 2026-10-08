# Crypto list — Expo (React Native + Skia)

The Expo port of the crypto list benchmark (`../SPEC.md`, especially "Decisions"). Bundle id
`dev.exact.cryptobench.expo`, display name "Crypto Expo".

## Versions
The same stack as `../../heavy-list/expo`: Expo SDK 58 `58.0.0-preview.6`, React Native
`0.88.0-rc.1`, React `19.3.0`, Hermes (default); built with Node 26 / npm 11.

| Package | Version | Why |
|---|---|---|
| `@shopify/react-native-skia` | 2.11.2 | Expo 58's `bundledNativeModules.json` |
| `@legendapp/list` | 3.4.0 | the list; pure JS, not in Expo's list, so the latest. Replaced `@shopify/flash-list` 2.0.2 on 2026-09-27 (per Charlie) |
| `react-native-reanimated` | 4.7.0 | Reanimated 4.6 (what preview.6 lists) declares RN 0.83–0.87; 4.7.0 declares 0.86–0.88 and is what `expo@58.0.0-preview.7` pins for RN 0.88.0-rc.1 |
| `react-native-worklets` | 0.13.0 | Reanimated 4.7's required worklets line (preview.7's pin) |
| `react-native-safe-area-context` | ~5.9.1 | top bar below the status bar |
| `expo-build-properties` | ~58.0.6 | iOS deployment target 17.0 |

`.npmrc` sets `legacy-peer-deps=true`: npm's semver does not count `0.88.0-rc.1` as inside
Reanimated/worklets' `0.86 - 0.88` peer range (prerelease), though it is the intended pairing.
No babel config: `babel-preset-expo` adds the worklets plugin itself.

## Shape (`App.tsx`)
- Coins live in React state (`useState(COINS)`, the JSON imported and inlined into the Hermes
  bundle by Metro). A tick copies the array and replaces only the 40 touched coin objects
  (new `series` array, `seq` counter, `up` flag), so `Row` (`React.memo`) re-renders only for
  changed coins.
- `LegendList` 3.4.0 with `keyExtractor`, a stable `renderItem`, `recycleItems` and
  `getFixedItemSize = 64` (its performance guide: fixed sizes when truly fixed); the default
  `maintainVisibleContentPosition` (ticks change values, never insert). It sits in its own `flex: 1`
  `View` (kept from FlashList; see gaps). One scroll view, content height 5,000 × 64 = 320,000 pt.
- Row: `View` 64 pt tall, 16 pt side padding, everything centred; 32 pt icon circle with the
  ticker's first letter (15 semibold white); name block `flex: 1, minWidth: 0` (16 semibold / 13
  #8E8E93, 2 pt gap, `numberOfLines={1}` tail ellipsis); chart; price block 104 pt right-aligned
  (price 16 semibold `fontVariant: ['tabular-nums']`, 4 pt gap, pill 12 semibold white, 2/6 pad,
  4 pt radius). Separator: an absolutely positioned full-width `StyleSheet.hairlineWidth` (1 px)
  #E5E5EA view at the row's bottom.
- Chart: a Skia `Canvas` per row, 114 × 50 pt at −9 pt margins (so 12 pt gaps around the 96 × 32
  box); the line is a `Skia.Path` built in `useMemo` from the series into the inner box, drawn by
  `<Path style="stroke" strokeWidth={1.5} strokeJoin/Cap="round" end={progress}>`. The dot
  (`Circle` r 3) and ring (`Circle` r/opacity from `useDerivedValue`) sit in a `Group` whose
  opacity is a shared value (0 until the draw-in ends).
- Draw-in: `useLayoutEffect` keyed on the **coin id** sets `progress` 0 → `withTiming(1, 600 ms,
  Easing.bezier(0, 0, 0.58, 1))`; its completion callback (a worklet, UI thread) shows the dot and
  starts `pulse = withRepeat(withTiming(1, 1,200 ms, same bezier), -1)`; ring r = 3 + 6·pulse,
  opacity 0.5·(1 − pulse). Mount and a recycled cell getting a new coin replay it; a tick (same
  id, new series) only rebuilds the path at full length and moves the dot, the pulse keeps phase.
- Flash: `Animated.Text` with `useAnimatedStyle` → `interpolateColor(flash, [0,1], ['#000',
  up ? green : red])`; on a new `seq` for the same coin: `flash = 1` then `withTiming(0, 400 ms,
  Easing.bezier(0.25, 0.1, 0.25, 1))` (CSS `ease`). A recycled cell resets the flash to 0.
- Ticks: `setInterval(100 ms)` from mount, `k` from 1, exactly the SPEC formulas
  (index `(k·7919 + j·104729) mod 5000`, δ, series shift, `change24h += δ·100`, `δ = 0` is up).
- Formatting: ≥ $1 `Intl.NumberFormat('en-US', 2 decimals)`; < $1 `toFixed(3 − floor(log10 p))`
  (4 significant digits, trailing zeros kept, no exponent). Pill `(c ≥ 0 ? '+' : '-') + |c|.toFixed(2) + '%'`.

## Launch environment (`modules/bench-env`)
Copied from `../../heavy-list/expo`'s local Expo module and extended: `BENCH_LIVE`, `BENCH_SCENARIO`,
`BENCH_FREEZE`, `BENCH_START_INDEX` from `ProcessInfo.processInfo.environment`. Ticks on iff
(`BENCH_LIVE=1` or `BENCH_SCENARIO=rest`) and `BENCH_FREEZE≠1`; the top bar says "Live: on/off"
accordingly. `BENCH_FREEZE=1`: draw-in complete from the first frame, ring static at r 6, opacity
0.25. `BENCH_START_INDEX=n`: LegendList `initialScrollIndex` (a real scroll; coin 500 lands exactly at the top).

## Build
```sh
bench/crypto-list/expo/build.sh      # ../prepare.sh, npm ci, expo prebuild (pod install), xcodebuild Release
```
`build-sim/CryptoExpo.app` (arm64 simulator) and `build/CryptoExpo.app` (device, unsigned: sign it with
`../../heavy-list/probe/resign.sh build/CryptoExpo.app dev.exact.cryptobench.expo`). `DEVICE_ONLY=1` skips the
simulator. Info.plist: MinimumOSVersion 17.0, UIDeviceFamily [1,2], UIRequiresFullScreen true, UIUserInterfaceStyle
Light. The measured build also had the Expo template's `assets/icon.png` as its app icon; `app.json` here names none.

## Smoke, LegendList 3.4.0 (iPad Pro M1, 2026-09-27)
Against the FlashList 2.0.2 build (series `ipad-r2`, round 1; ladder round 2):
- fling 0: 113–114/109–110/96–100/92–96/92 fps at 1k/3k/6k/12k/24k (FlashList 113/108–110/100–108/98–106/95–105),
  main 431 ms/s (416), peak 149 MB (152). FlashList's +6k/+12k/+24k segments travelled only 50 %, so its
  higher positive-direction numbers are for half the distance; LegendList travelled 100 % everywhere.
- rest 0: 117.3 fps, main 369 ms/s (FlashList 115.6, 403).
- ladder 0: 108–109 at ±3k, 96–98 at ±6k, 93–95 at ±12k, 89–91 at ±24k, 80 at ±48k, 83–88 at ±96k
  (FlashList 107–108, 99–107, 97–106, 95–105, 81–97, 76–96, positive halves at 50 % travel). Breaking
  point 3000 (fps 108) for both.
- fling 4 (`BENCH_RENDER=layer`, as r2's `fling-b`): no blanks to ±3k, 1–3/38 samples at 6k–12k, 10–18/35 at 24k,
  bands to 872 pt (FlashList: none to 6k, 1–2/40 at 12k, 6–15/37 at 24k, to 948 pt); sampled fps 69–80 (73–83).
  (The same run with the default hierarchy renderer gave 40–47 fps; not comparable.)
- Parity: the frozen top of the list matches FlashList's; with `BENCH_START_INDEX=500` coin 500 (Draeium) is exactly
  at the top.

## Gaps and findings
- The findings below about FlashList (`estimatedItemSize`, `initialScrollIndex`) are the 2026-09-26 build's;
  that build is not kept.
- **FlashList v2 has no `estimatedItemSize`** (removed in 2.0; it measures items itself), so it is
  not passed. The probe's "tallest UIScrollView with contentSize.height > 20000" still finds the list.
- **FlashList 2.0.2 `initialScrollIndex` is off by the list's offset inside its parent** when the
  list has a sibling above it (the top bar): coin 500 landed ~41 pt low. Wrapping the list in its
  own `flex: 1` View fixes it (coin 500 exactly at the top); `onLoad` + `scrollToIndex` did not.
- The recycled-row draw-in replay and the pulse phase were checked by reading the code, not
  observed while scrolling.
  The flash is too short to catch in a 5 s screenshot; the tick itself was observed (prices,
  pills, series and a colour flip on Rhoonatum after 5 s).
- Pill text for a tiny negative change (e.g. −0.004) prints `-0.00%` (sign from `change24h`).
- Hermes `Intl` rounds half away from zero, not half-even (SPEC accepts differences at exact halves).
- The heavy app actually shipped `UIRequiresFullScreen` false; this one sets it true as requested.
