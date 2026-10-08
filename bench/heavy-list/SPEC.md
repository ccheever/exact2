# Heavy list benchmark — the row, the screen, the rules

Four apps render the SAME screen from the SAME data: SwiftUI (baseline, the most ordinary
SwiftUI), exact2 (Contract, `list virtualized=true`), Expo (`@expo/ui` SwiftUI components), and
UIKit (hand-written, added 2026-09-29; see "UIKit" below). The probe (`probe/probe.m`) and harness
began as the easy list benchmark's and were extended here. How to build and run: `README.md`.

## Data
`data/messages.json` (from `gen.py`, deterministic; never regenerate per app) and
`data/images/*.jpg` (`prepare.sh` runs it and checks the output against `data.sha256`). Every app bundles both and reads them at startup; no network.
Message fields: `id`, `index`, `author`, `avatar` (file), `minutesAgo`, `paragraphs`
(array of paragraphs; each an array of runs `{t, s?}` with `s` ∈ bold | italic | code |
link | mention | tag, absent = plain), optional `photos` [{src,w,h}] (1–4), `link`
{thumb,title,description,site}, `quote` {id,author,excerpt}, `reactions` [{emoji,count}].

## Screen
One vertical list of all 10,000 messages, oldest first (index 0 at top). Plain list style
(no inset cards, no sections). A top bar above the list (not in it): title "Heavy list",
a live-mode label "Live: off/on". Background white; rows separated by a 0.5 pt hairline
#E5E5EA inset 68 pt from the left.

## Row (all sizes in points; fonts are the iOS system font at these sizes)
Row padding: 12 top, 12 bottom, 16 left, 16 right. Horizontal: avatar column + content column, gap 12.

1. **Avatar**: 40×40 circle, the `avatar` JPEG, aspect-fill, decoded at 40×40@screen scale.
2. **Header line** (content column top): author 15 semibold #000; then gap 6; then relative
   time "3m ago"/"2h ago"/"4d ago" (from `minutesAgo` + seconds since launch, updated once a
   second in live mode, static otherwise) 13 regular #8E8E93. Single line.
3. **Quote** (if present), 6 below header: a box with 3-pt left bar #C7C7CC, background
   #F2F2F7, corner radius 8, padding 8; inside: author 13 semibold #3C3C43, then excerpt
   13 regular #3C3C43, max 2 lines, truncated with an ellipsis.
4. **Paragraphs**, 6 below the previous element, 8 between paragraphs. Each paragraph is ONE
   text flow of its runs (wrapping across runs), 16 regular #000, line height default:
   - bold: semibold; italic: italic; code: monospaced 15, background #F2F2F7 (inline);
     link: #007AFF underlined; mention: #007AFF semibold; tag: #5856D6. Emoji and
     non-Latin text are plain runs (RTL scripts shape as the platform does).
5. **Photos** (if present), 8 below: width = content column width, radius 12, clipped;
   - 1 photo: aspect of the image, height capped at 320 (aspect-fill within the cap);
   - 2: side by side, gap 4, each square;
   - 3: one large left (2/3 width, square), two stacked right (1/3 width), gap 4;
   - 4: 2×2 grid of squares, gap 4.
   Decode downsampled to the displayed size (never the full 1600 px) off the main thread;
   a #E5E5EA placeholder until decoded.
6. **Link card** (if present), 8 below: border 0.5 #D1D1D6, radius 12, clipped; thumbnail
   full card width × 140 (aspect-fill); below it padding 10: site 12 regular #8E8E93 uppercase,
   title 15 semibold #000 max 2 lines, description 13 regular #3C3C43 max 2 lines (ellipsis).
7. **Reactions** (if present), 8 below: a wrapping row of chips, gap 6 both ways; chip:
   height 28, padding 10 horizontal, radius 14, background #F2F2F7, emoji 14 + gap 4 + count
   13 semibold #3C3C43.

No gestures are required in rows except: tapping a reaction chip increments its count (app
state keyed by message id + emoji). Accessibility labels: row = "<author>, <time>".

## Live mode (a stress scenario)
Enabled by launch environment `BENCH_LIVE=1` (read at startup; SwiftUI/exact2 via
ProcessInfo/host env, Expo via a native constant or a launch argument the port finds).
When on:
- every 250 ms insert one new message at the TOP (index −1, −2, …, content copied from
  message (k*37) % 10000 with a new id `live-k`, `minutesAgo` 0) — the list must keep the
  user's current position stable (content under the viewport does not jump);
- every 250 ms bump one reaction count on a message currently near the viewport (pick by
  index = (k*101) % 10000 if you cannot know visibility — deterministic is fine);
- the relative times of visible rows update once a second.

## Parity rules
- Same data, same images, same sizes, same fonts and colours. If a framework cannot express
  an item, drop it NOWHERE silently: record the gap in the port's README and in its report;
  do not substitute a cheaper element without saying so.
- The most ordinary idiomatic way to build a list in each framework: SwiftUI `List`
  (`.listStyle(.plain)`) + `ForEach`; exact2 `list virtualized=true` + `each … key=`;
  Expo `@expo/ui` `List` + `List.ForEach` with its data/recycling API (PR 49975).
- Release builds. Bundle ids: `dev.exact.heavybench.swiftui`, `.exact`, `.expo`, `.uikit`.
- The probe finds the tallest `UIScrollView`; each app must have exactly one large list scroll view.

## UIKit (added 2026-09-29, `uikit/`, `dev.exact.heavybench.uikit`)

A fourth competitor written the way a strong iOS engineer writes a performance-sensitive feed in UIKit
(not the "most ordinary" bar the other three follow): Charlie's fairness rule applies — the same content and
the same work, no skipped items, lower quality or cheaper placeholders — and idiomatic UIKit optimizations are
the point. Equivalence decisions, item by item (details in `uikit/README.md`):

| Spec item | UIKit |
|---|---|
| list | `UICollectionView` + compositional layout (full-width items, estimated 330 pt, 20 header-less sections of 500 so a self-sizing re-solve stays in one section) + diffable data source (Int ids) |
| row sizing | self-sizing via `preferredLayoutAttributesFitting` from a hand-computed row layout; no Auto Layout in cells |
| row layout / rich text | built off the main thread in `prefetchItemsAt` (attributed strings + `boundingRect`), `NSCache` 400; main-thread build on a miss |
| paragraphs | one `UILabel` + `NSAttributedString` per paragraph, the same run styles as SwiftUI's `AttributedString` |
| images | `preparingThumbnail(of:)` at the aspect-fill pixel size, off main, `NSCache` 300 (as SwiftUI's), prefetched; #E5E5EA placeholder |
| quote / link card | `UIView`s with `cornerRadius` + `clipsToBounds`, labels clamped to 2 lines with ellipsis |
| reactions | `UIControl` chips, hand-flowed, gap 6; tap bumps the count and reconfigures the item |
| separator | 1 px #E5E5EA in the cell, 68 → width − 16 (the span SwiftUI's system separator has) |
| live inserts | snapshot insert at the top, no animation (costs a whole-layout re-solve, one late frame per insert; see `uikit/README.md`), then the content offset is corrected so the first visible row stays put (**keeps position**, which the spec asks and SwiftUI's `List` does not) |
| live bump | model update; reconfigure only if the row is on screen (off-screen rows re-layout when dequeued) |
| live times | the visible cells' time labels set once a second |
| focus | `allowsFocus = false` and a `UICollectionView` subclass whose `focusItems(in:)` returns none: an iPad with a hardware keyboard otherwise walks every visible cell on each focus update while scrolling (traced in the crypto UIKit app; exact2's host does the same, `FocusSearchIOS.swift`); taps are unaffected |
| `BENCH_START_INDEX` | `scrollToItem(at:at: .top)` three times 250 ms apart (estimated heights settle), as SwiftUI's app repeats |

The first series with UIKit (2026-09-29, 3 rounds, Expo in round 1 only) measured exact2 at origin/main
dedbe6e8.

## Cold start measurement (2026-09-29)

**Cold start values before 2026-09-29 ~20:00 are inflated.** The probe's own static initializer (a Metal hook that
scanned every ObjC class) ran pre-main in every app, more so in exact2's larger binary, and injecting the probe with
`DYLD_INSERT_LIBRARIES` also disables the app's prebuilt launch closure. The coldstart columns of every series taken
with the old probe (every series through 2026-09-29 afternoon) are therefore not comparable across apps. Cold start is now
measured with the fixed probe **linked** into a copy of each app (`probe/linkprobe.py` adds an `LC_LOAD_DYLIB`;
`probe/makecold.sh`, bundle ids `…<app>cold`), launched without insertion by `probe/devrun-cold.sh`, 3 interleaved
rounds per device (`cold-series.sh`), summarized by `coldsum.py`.
Scroll fps and CPU from the older series stand; their memory carries a small fixed probe offset (about +11 MB exact2,
+7.5 MB SwiftUI).

The first results with the linked probe are in `README.md`, "Last standings".
