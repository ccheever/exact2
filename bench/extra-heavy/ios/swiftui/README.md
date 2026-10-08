# Extra Heavy feed — SwiftUI app (iOS)

`App.swift` (data, list, header, clocks), `Kinds.swift` (the first 17 row bodies), `Nested.swift` (filmstrip, inbox), `Media.swift` (image loader,
video, animated images, Lottie, web view), `Shader.metal` (the shader row). `./build.sh` builds
`build/XHeavy.app` (iphoneos, unsigned); `SIM=1 ./build.sh` also builds `build-sim/` (ad-hoc signed) for a
simulator. `../shoot-sim.sh swiftui <name> <delay> [ENV=VAL…]` relaunches it on `BENCH_SIM` and screenshots it.

Build notes:
- No Xcode project: `swiftc` over the four files, `actool` for `obj/Assets.xcassets` (every `../../data/svg/*.svg`
  as a vector imageset), `UIAppFonts` for `../../data/fonts/*.ttf`, all data files flat at the bundle root.
- `Shader.metal` → `default.metallib` is compiled by `metal.sh` with the Metal Toolchain (this Mac's, or
  `BENCH_METAL_HOST`'s over ssh); `build.sh` calls it when the shader is newer than `obj/default-<sdk>.metallib`.
- Lottie: `../../vendor/lottie-ios` (airbnb/lottie-ios `2c8608c`, 2026-09-19, `Sources/`; `../../fetch-lottie.sh`) compiled once per
  SDK into `obj/lottie-<sdk>/libLottie.a` + `Lottie.swiftmodule` (~35 s).
- Device: `../../device/XHeavy.app` = the build signed with the probe by `../../../heavy-list/probe/resign.sh … dev.exact.xheavy.swiftui`.

What each kind uses: see the SPEC's Decisions table (SwiftUI column). Specifics:
- `List(.plain)` + `ForEach(rows)`, one `switch` on kind; the row content is `frame(width: C)` centred.
- Nested lists (kinds 18, 19): `ScrollView(.horizontal) { LazyHStack }` and a 400 pt `ScrollView { LazyVStack }` in the row; the
  offset per row id in `ScrollMemory` (plain class) via `ScrollPosition` + `onScrollGeometryChange` (iOS 18, `#available`).
  Captions use `Text(verbatim:)`: `Text("IMG_\(n)")` localises the Int to `IMG_3,309`.
- Clocks: `Timer.publish(every: 1)` → `seconds` (s) passed to every row; `TimelineView(.animation)` inside the
  shader, rings and waveform (t). `BENCH_FREEZE` swaps each TimelineView for a fixed t.
- Thread expansion and comment drafts live in an `@Observable RowState` keyed by row id.
- Shimmer: `LinearGradient` band offset by `.linear(duration: 1).repeatForever`, masked by the cell grid;
  the 1.2 s skeleton gate is a `.task(id: row.id)` sleep.

- `BENCH_START_INDEX`: `scrollTo(anchor: .top)` repeated 3× 250 ms apart — one call lands a row or two off
  because List estimates unmeasured heights.

Smoke (iPad, 2026-09-27, data version 2):
- fling 0: 113/108/104–110/106–110/91–100 fps at 1k/3k/6k/12k/24k pt/s, main 414 ms/s, peak 303 MB.
- rest 0: 119.9 fps, main busy 384 ms/s.
- innerfling 0: the strip holds 119.9 fps at every ladder speed up to ±96k pt/s, and the inbox 117–120
  (busy 3.1–4.5 ms/frame). innerfling 4 (sampled): no blank band inside either inner list at any speed.
- innerkeep 0: all four marks kept (strip 12,345 / 23,456, inbox 5,432 / 8,765) with a new
  `UIScrollView` each time (`sameView` false): `ScrollPosition` restored them. Feed flings at 6k/12k were
  102–116 fps.

Known divergences / gaps:
- **Wrapped RTL paragraphs**: the Arabic block's continuation line is not right-aligned. Tried
  `layoutDirection = .rightToLeft` + `.leading`, `.multilineTextAlignment(.trailing)` + trailing frame, and
  natural (`.leading`) alignment — three rounds, same result; recorded, not fixed. Single-line RTL blocks are
  right-aligned correctly.
- Map marker is `Marker` (balloon); Expo's default is a pin (SPEC Decisions).
