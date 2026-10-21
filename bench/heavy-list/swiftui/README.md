# Heavy list — SwiftUI baseline

`App.swift` is the whole app; `./build.sh` builds `build-sim/HeavyBench.app` (arm64 simulator,
ad-hoc signed) and `build/HeavyBench.app` (iphoneos, unsigned). Both bundle `../data/messages.json`
and `../data/images/*.jpg` flat at the bundle root (`../prepare.sh` makes them). `shoot.sh` relaunches
on the simulator `BENCH_SIM` and screenshots to `target/bench/heavy-list/shots/`.

Env: `BENCH_LIVE=1` (live mode), `BENCH_START_INDEX=n` (ScrollViewReader.scrollTo at launch).

## Choices
- List: `List` + `.listStyle(.plain)` + `ForEach($messages)` over `@State [Message]`.
- Rich text: one `Text(AttributedString)` per paragraph; each run is an `AttributedString`
  with `font` / `foregroundColor` / `backgroundColor` / `underlineStyle`, concatenated. Built in
  the row body (not cached). Links are styled, not tappable (no `.link` attribute).
- Images: custom `BundleImage` view, `.task(id:)` → `Task.detached` →
  `CGImageSourceCreateThumbnailAtIndex` at the aspect-fill pixel size, `NSCache` (300 entries).
  #E5E5EA placeholder until decoded. `AsyncImage` rejected: it decodes full size (no downsample).
- Reactions: minimal custom `Layout` (`FlowLayout`, gap 6 both ways); chips are borderless
  `Button`s that increment the count in the message (app state keyed by id + emoji index).
- Photo/link widths come from a `GeometryReader` around the List (width − 84).

## Gaps / deviations
- **Live inserts do not keep position.** A plain `List` keeps its content offset when rows are
  inserted above, so what is on screen moves down one row per insert (started at 476, the top
  row is 456 after 5 s). `.scrollPosition(id:anchor:)` on `List` has no effect (iOS 27 sim).
  A ScrollViewReader compensation (scroll the top on-screen row, tracked by onAppear/onDisappear,
  back to the top after each insert) was tried in three rounds and did not hold (same
  update: no-op; deferred one turn: overshoots, 476 → 480 in 5 s) and would fight user drags;
  it is kept for reference in `App.anchor-attempt.swift.txt`, not built.
- Separator: the system hairline (1 px) tinted #E5E5EA, inset 68 via
  `alignmentGuide(.listRowSeparatorLeading)`; not an exact 0.5 pt line.
- Reaction bump: index (k*101) % 10000, or the next message after it that has reactions.
- Relative time for "0 minutes" renders "0m ago".
