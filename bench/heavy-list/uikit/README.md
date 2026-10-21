# Heavy list — UIKit

`App.swift` is the whole app (hand-written UIKit, no storyboard, scene lifecycle); `./build.sh` builds
`build-sim/HeavyUIKit.app` (arm64 simulator, ad-hoc signed) and `build/HeavyUIKit.app` (iphoneos, unsigned),
bundle id `dev.exact.heavybench.uikit`. Both bundle `../data/messages.json` and `../data/images/*.jpg` at the
bundle root, as the SwiftUI app does (`../prepare.sh` makes them). Device: `build/HeavyUIKit.app` re-signed by
`../probe/resign.sh build/HeavyUIKit.app dev.exact.heavybench.uikit`.

Env: `BENCH_LIVE=1` (live mode), `BENCH_START_INDEX=n` (`scrollToItem(.top)`), `BENCH_TIMING=1` (logs the
live step's snapshot/apply/anchor timings; diagnostics only).

## How it is built (the way a performance-minded iOS engineer writes a feed)
- **List**: `UICollectionView` with a `UICollectionViewCompositionalLayout` (one full-width item per group,
  `.estimated(330)` height) and a `UICollectionViewDiffableDataSource<Int, Int>` (Int item ids: the message
  ordinal, live inserts 1,000,000 + k), the 10,000 rows in **20 header-less sections of 500** (`BENCH_CHUNK`
  overrides; 0 = one section). One reused cell class. Why sections: with one 10,000-item section, every
  self-sized cell makes the compositional layout re-solve all the estimated items after it
  (`_UICollectionCompositionalLayoutSolver resolveForInvalidatedPreferredAttributes`, 60–70 % of the main thread
  at 12k–24k pt/s on the simulator); in sections it re-solves one section and shifts the rest. Simulator fling at
  24k pt/s: busy 13.6 → 3.1 ms/frame. Invisible on screen (no headers, no spacing).
- **Cells**: plain `UICollectionViewCell`, subviews created once and reused (paragraph labels, photo views and
  chips grow on demand and are hidden when unused), laid out by hand in `layoutSubviews` from a `RowLayout`
  (every frame of the row + the paragraphs' attributed strings), self-sizing through
  `preferredLayoutAttributesFitting` returning the layout's height. No Auto Layout inside cells.
- **Row layout off the main thread**: `UICollectionViewDataSourcePrefetching` builds `RowLayout`s on a
  background queue (attributed strings + `boundingRect` measurement, thread-safe) into an `NSCache` (400);
  a miss at dequeue builds it on the main thread.
- **Text**: `UILabel` with an `NSAttributedString` per paragraph (the SwiftUI app's run styles:
  semibold, italic, monospaced 15 on #F2F2F7, link blue underlined, mention blue semibold, tag purple);
  quote excerpt and link title/description `numberOfLines = 2` with tail truncation.
- **Images**: `UIImage(contentsOfFile:)` → `preparingThumbnail(of:)` at the aspect-fill pixel size of the
  displayed box (never the 1600 px original), on a concurrent `.userInitiated` queue; an `NSCache` of 300
  (the SwiftUI app's size); requests de-duplicated; prefetch starts decodes for avatars, photos and link
  thumbnails. #E5E5EA placeholder (the view's background) until decoded.
- **Reactions**: `UIControl` chips (emoji 14 + count 13 semibold) in a hand-computed flow (gap 6); a tap bumps the
  count, bumps the message's `rev` (new layout) and reconfigures that item.
- **Separator**: a 1-px #E5E5EA view in the cell, from x = 68 to width − 16 (the SwiftUI List's system separator
  spans the same; 1 px, as SwiftUI's).
- **Live mode**: a 250 ms `Timer`: insert at the top with a snapshot `insertItems(_:beforeItem:)`, applied without
  animation; then **position keeping**: the first visible item's y before and after the apply fixes the
  content offset, so what is on screen stays still (the SwiftUI app cannot, see its README). The reaction bump
  reconfigures the item only when it is on screen (an off-screen row picks up the new layout when dequeued).
  Once a second the visible cells' time labels are set directly (no reconfigure).

- **Focus**: `allowsFocus = false` + `FeedCollectionView.focusItems(in:)` → `[]` (see SPEC; the iPad's hardware
  keyboard makes UIKit's focus system re-scan visible cells while scrolling). Chips stay tappable; they are not
  reachable by keyboard focus, as in exact2's host.

## Gaps / deviations
- None in content. The top-bar divider is the system `.separator` colour, 1 px, as SwiftUI's `Divider`.
- Paragraph text sits within about a point of SwiftUI's per paragraph (UILabel vs SwiftUI `Text` line boxes);
  over a screen rows drift by a few points (checked with parity screenshots on 2026-09-29).
- **The live insert is expensive**: every diffable apply that inserts an item makes the compositional layout
  re-solve every section's estimated items (`_UICollectionLayoutSectionEstimatedSolver`) on the main thread,
  whatever the sectioning (sim: 17–19 ms with 500-row sections, ~28 ms with 20-row ones, 25 ms with one section;
  inserting each live row as its own new section is no cheaper). On the iPad and iPhone that is one 55–75 ms
  frame per insert, i.e. four late frames a second in live mode. Separating the bump's reconfigure (only for an
  on-screen row) halved it from two layout passes to one. A hand-written `UICollectionViewLayout` with a height
  prefix array would make the insert O(1)-ish; it was not built, because the brief is compositional layout +
  diffable data source.
