# Heavy list — Expo (`@expo/ui`)

The Expo port of the heavy list benchmark (`../SPEC.md`). Bundle id `dev.exact.heavybench.expo`.
Same stack as the easy list benchmark's Expo port: Expo SDK 58 `58.0.0-preview.6`,
RN `0.88.0-rc.1`, `@expo/ui` `58.0.6` (List.ForEach data/recycling API, PR 49975), plus
`expo-image` 58.0.6. Created with `create-expo-app --template blank-typescript@next`, `expo`
pinned back to preview.6 to match the easy benchmark.

## Shape
- `App.tsx` — the whole screen: one `Host`, a top bar (`HStack` + `Divider`), `List` with
  `listStyle('plain')`, one `List.ForEach` with `data` / `keyExtractor` / a `useCallback` row
  render function (recycling on, the default; `estimatedItemSize={160}`, default overscan 10).
  Row insets, separator tint #E5E5EA, white row background are modifiers on the ForEach; the
  separator inset (68 pt) is `alignmentGuide('listRowSeparatorLeading', 52)` on the row.
- `data/messages.json` — copied from `../data` by `../prepare.sh` (not committed), imported by `App.tsx`
  (Metro inlines it into the Hermes bundle; that is the ordinary Expo way to ship JSON).
- `assets/images/*.jpg` — copied from `../data/images` by `../prepare.sh`; `images.ts` maps each
  file name to a `require()` so Metro bundles all 104.
- `modules/bench-env` — a tiny local Expo module (autolinked from `modules/`) exporting
  `BENCH_LIVE` and `BENCH_START_INDEX` from `ProcessInfo.processInfo.environment` as module
  constants. So `devicectl … --environment-variables '{"BENCH_LIVE":"1"}'` and
  `SIMCTL_CHILD_BENCH_LIVE=1 xcrun simctl launch …` both work.

## What `@expo/ui` 58.0.6 could and could not express

| Spec item | How it is built | Gap |
|---|---|---|
| Paragraph as one text flow of styled runs | One `Text` with nested `Text` children (SwiftUI `Text + Text` concatenation) | Nested `Text` accepts only `bold`, `italic`, `font`, `foregroundColor/Style`, `monospacedDigit` (`applyTextModifier`). **No per-run background**: inline `code` has no #F2F2F7 fill. **No per-run underline**: links are #007AFF but not underlined. `markdownEnabled` does not help (Markdown has neither). No AttributedString API. |
| bold / italic / mention / tag / code font | `font({size:16, weight:'semibold'})`, `italic()`, `foregroundStyle`, `font({size:15, design:'monospaced'})` | — |
| `lineLimit` (quote excerpt, link title/description), header single line | `lineLimit(n)` modifier | — |
| Rounded clipping, backgrounds, quote bar, card border | `clipShape('circle' / 'roundedRectangle', r)`, `background(color)`, `background(color, shapes.roundedRectangle)`, `Overlay` + `Rectangle` for the 3-pt bar, the `strokeBorder({shape:'roundedRectangle', cornerRadius:12, …})` modifier for the 0.5-pt card border | — |
| Images (avatar, photos, link thumb) | **expo-image** `Image` inside `RNHostView`, inside a SwiftUI `ZStack` with a fixed `frame` and SwiftUI clip | `@expo/ui`'s own `Image` takes only `uiImage: 'file://…'`, which it reads **synchronously on the main thread, full size, inside `body`** (`Data(contentsOf:)` + `UIImage(data:)`, re-run on every body evaluation) — it cannot downsample or decode off the main thread, and has no placeholder. The spec requires both, so the port mixes in expo-image (downsampled to the view size by default, decoded off the main thread by SDWebImage, `recyclingKey` for recycled rows, #E5E5EA `backgroundColor` as the placeholder). `RNHostView` is Expo's documented way to put RN views inside `Host`; Expo's own universal `ListItem` does the same. This is what an Expo developer would do, and it means each image is a React Native view hosted in the SwiftUI row. |
| Photo sizes | Computed in JS from `Dimensions.get('window').width - 84` (the content column), same formulas as the SwiftUI baseline | — |
| Reaction chips, wrapping row with gap 6 | Chips are `Button` (`buttonStyle('borderless')`) around an `HStack`; lines are `HStack`s in a `VStack` | **No wrapping layout** in `@expo/ui` (no `Layout` protocol, no flow). The port breaks chips into lines in JS, greedily, from an **estimated** chip width (emoji 19 pt, 8.3 pt per digit, + 24). Line breaks can differ from a measured flow near the edge. |
| Tap a chip → count + 1 | `bumps` state keyed `id|emoji` | — |
| Row accessibility label "<author>, <time>" | `accessibilityElement('contain')` + `accessibilityLabel` on the row | — |
| Top bar | `HStack` + `Divider` above the `List`, inside the same `Host` | — |

## Live mode (`BENCH_LIVE=1`)
Read from the launch environment by `modules/bench-env` (native `ProcessInfo`, exported as an
Expo module constant; empty when unset). When on, every 250 ms: insert `live-k` (copy of original
message `(k*37) % 10000`, `minutesAgo` 0) at the top of `data`; bump reaction `k % n` of original
message `(k*101) % 10000` (or the next one with reactions), the same choice as the SwiftUI
baseline; every 4th step the seconds counter ticks, which re-renders the visible rows' times.

Gaps and findings:
- **Position is not held.** `@expo/ui`'s only programmatic scroll is the `scrollPosition(id:)`
  modifier; SwiftUI's `List` ignores it (tried with an initial id, with `scrollTargetLayout()` on
  the ForEach, and by writing the id after mount — the list stayed at the top). So the inserted
  rows push the content down, as a List that keeps its content offset does.
- **The rows go blank under live mode.** Each insert replaces `data`, which bumps the ForEach's
  revision: every slot hides its content (placeholder at the measured height) until JS re-renders
  it, and because slots are assigned by `index % slotCount`, a top insert moves every item to a
  different slot. At 4 inserts/s on an M5 Mac mini's simulator the rows never catch up (≈190 % CPU,
  every visible row a blank placeholder by 10 s).
  A diagnostic build with a 2 s interval (not kept) filled every row and went idle between ticks,
  so this is throughput, not a hang.

## `BENCH_START_INDEX`
`@expo/ui`'s `List` has no scroll-to-item (see above), so it cannot scroll to an index at launch.
For screenshots only, `BENCH_START_INDEX=n` **slices** `data` to start at message `n` — a
different list, not a scroll. Never set it for timing runs.

## Build

`./build.sh` runs `../prepare.sh`, `npm ci`, `expo prebuild --platform ios` (which runs `pod
install`; `ios/` is generated and not committed) and two Release `xcodebuild`s, unsigned:
`build-sim/HeavyBenchExpo.app` (simulator) and `build/HeavyBenchExpo.app` (device; sign it with
`../probe/resign.sh` before installing). `DEVICE_ONLY=1` skips the simulator build. It needs Node
and npm, CocoaPods, and Xcode; the build was made on 2026-09-25 with Xcode 27 and has not been
rebuilt since.
