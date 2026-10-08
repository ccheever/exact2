# Extra Heavy feed benchmark — spec

A social feed whose rows hold "all sorts of crazy stuff" (Charlie, 2026-09-27): a live GPU
shader over a photo, canvas drawings, SVGs, muted autoplay video, maps, Markdown, a wide variety
of fonts, nested horizontal carousels, animated GIF/WebP and Lottie, glass over images, complex
scripts, live counters and rings, rows that change height, skeleton shimmer, embedded web views
and text inputs — and, as a deliberately extreme stress test (Charlie, 2026-09-27), rows that hold
their own **nested virtualized list**: a horizontal strip of 2,000 thumbnails, and a 400 pt box
holding a vertical list of 1,000 messages. Every app renders the same rows from the same data, driven by the same probe
(`../heavy-list/probe`), on the M1 iPad Pro 12.9" (120 Hz), which stands in landscape
(1366 × 1024 pt).

The easier benchmarks are `../heavy-list` (text + photos) and the crypto list (a live
chart per row); this one reuses their probe, scenarios, seed and conventions.

## Apps

| App | Bundle id | Built with |
|---|---|---|
| SwiftUI | `dev.exact.xheavy.swiftui` | `List` + system views; `ios/swiftui/` |
| Expo | `dev.exact.xheavy.expo` | React Native + LegendList + community modules; source in `expo/` |
| Exact2 | `dev.exact.xheavy.exact2` | `list virtualized=true` + Contract, a Rust data source, a GPU module; `exact-xheavy/` |
| UIKit | `dev.exact.xheavy.uikit` | hand-written UIKit: `UICollectionView` (compositional layout, diffable data source, prefetching) + system views and layers; `ios/uikit/` (added 2026-09-29) |

Each is written the most ordinary way for its stack (see Decisions), not tuned against the others.

## Data (`gen.py`, seed 49975) and assets

`gen.py` writes `data/feed.json` = `{"version":2,"kinds":[…],"fonts":{…},"messages":[…],"rows":[…]}` and the assets
below. Every app bundles `data/` (or the part it uses) and reads it at startup; **no network at
runtime except the map tiles** (below). Never regenerate per app.

| Asset | What | Made by |
|---|---|---|
| `images/photo-00…39.jpg` | large photos, long side 1600 px, aspects 4:3, 3:4, 16:9, 1:1, 9:16, 3:2 (q 85) | PIL (the heavy bench's synthetic photo) |
| `images/small-00…47.jpg` | small images, 256 × 256 | PIL |
| `images/avatar-00…23.jpg` | avatars, 256 × 256 | PIL |
| `anim/sticker-00…05.gif` | animated GIF, 240 × 240, 12 frames × 100 ms, loop forever | PIL |
| `anim/eq-00…05.webp` | animated WebP (lossy q 80), 240 × 240, 16 frames × 80 ms, loop forever | PIL + libwebp |
| `anim/motion-00…03.json` | Lottie (Bodymovin 5.7.4), 240 × 240, 60 fps, 120 frames, 3 shape layers | `gen.py` (hand-built JSON) |
| `video/clip-00…03.mp4` | H.264 Main, 640 × 360, 30 fps, 4.0 s, no audio | `gen_video.swift` (AVFoundation; ffmpeg is broken on this Mac) |
| `svg/icon-*.svg` (12), `svg/chart-00…23.svg`, `svg/art-00…11.svg` | icons (24 viewBox, fill #3C3C43), bar + line charts (600 × 160), landscape illustrations with linear gradients (600 × 300) | `gen.py`; also all in `svgs.json` (name → XML) |
| `embed.html` | the web-view page template (placeholders `{{HUE}} {{TITLE}} {{N}} {{PLAY}} {{BARS}}`) | `gen.py` |
| `fonts/*.ttf` | Bebas Neue, Pacifico, DM Serif Display, Abril Fatface, Space Mono (Regular, Bold), Lobster, Special Elite, Permanent Marker, Crimson Text (Regular, Italic), Inter — OFL / Apache 2.0, licences beside them, SHA-256 in `SHA256SUMS` | `fetch-fonts.sh` (once; an input, not regenerated) |

The video encoder's bytes can differ run to run; the pixels fed to it cannot. `gen.py` skips the
clips when they exist.

Row fields (all rows): `id` (`r0`…), `index`, `kind`, `author`, `handle`, `avatar`, `minutesAgo`;
the kind's payload below. 3,000 rows, 19 kinds:

- **Cadence of the nested kinds.** Row i is a `filmstrip` iff i mod 6 = 3 (rows 3, 9, 15, …: 500 rows)
  and otherwise an `inbox` iff i mod 12 = 7 (rows 7, 19, 31, …: 250 rows). So a nested list starts
  every 4 rows on average; one or two are on every landscape screen.
- **The other 2,250 rows** are the first 2,250 rows of the 17-kind stream, in order, with their
  content unchanged: kinds cycle in blocks of 17, block 0 the fixed order `photo live shader carousel
  motion video map markdown code intl typeface svg glass thumbs thread webview canvas`, every later
  block a seeded permutation of the 17. Each of the 17 appears 132–133 times.
- **Stability (2026-09-27, `version` 2).** Adding the nested kinds left every asset byte-identical and
  every 17-kind row's payload identical (checked field by field against version 1). What changed:
  those rows' `id` and `index` (renumbered in the interleaved order, so `r3` is now a filmstrip),
  the typeface font (still from the row's *version-1* block, so it no longer steps every 17 rows),
  the thread flip parity (it uses `index`), and version 1's rows 2,250–2,999, which are gone. The
  nested rows draw from their own stream (seed 49975 + 1,000,000), so the 17-kind stream is the
  version-1 stream exactly. A nested row's `minutesAgo` is the previous row's.
- **`messages`** is the inbox's pool: 997 `{name, avatar, text}`.

## Screen

- A top bar above the list (not in it), as in the other benchmarks: "Extra Heavy" 17 pt semibold #000
  left, "Live: on"/"Live: off" 13 pt #8E8E93 right, padding 10 pt vertical / 16 pt horizontal, a 0.5 pt
  #E5E5EA hairline below it.
- One vertical list of the 3,000 rows in data order (`r0` at top), plain style, white background,
  light mode, no section headers. The feed is the one large vertical scroll view. The probe picks, among
  scroll views ≥ 300 pt tall, the one with the largest content height, which must be > 20,000 pt. The
  feed's is about 1.4 M pt. The nested ones are smaller: carousels, strips and web views are < 300 pt
  tall; an inbox is 400 pt tall with about 66,000 pt of content.
- Every row: white, 16 pt top and bottom padding, a full-width 0.5 pt #E5E5EA separator at its bottom.
  The row's **content column** has width **C = min(list width − 32, 600)** and is centred horizontally
  (on the landscape iPad C = 600).
- Colours used throughout: text #000, secondary #8E8E93, label-2 #3C3C43, fill #F2F2F7,
  hairline #E5E5EA, border #D1D1D6, blue #007AFF.
- Fonts are the iOS system font unless a bundled font is named. Sizes are points.

## Row header (every kind)

Horizontal, vertically centred, 36 pt tall: avatar 36 × 36 circle (the avatar JPEG, aspect-fill,
decoded at display size) · 10 gap · a column (2 pt gap): author 15 semibold #000, then
`{handle} · {rel}` 13 regular #8E8E93 (single lines, tail ellipsis) · flexible space · the kind name
uppercased (`MAP`, `WEBVIEW`…) 11 semibold #8E8E93. The body starts 10 pt below the header;
the body's own elements are separated by 10 pt unless the kind says otherwise.

`rel` from m = `minutesAgo` + ⌊s / 60⌋ (s = the live clock, below): m = 0 → `now`; m < 60 → `{m}m`;
m < 1440 → `{⌊m/60⌋}h`; else `{⌊m/1440⌋}d`.

## Row kinds (the body; W, H are the element's box)

1. **photo** — `caption` 15 regular #000 (wraps). Then the photo: width C, height min(C·h/w, 1.25·C),
   aspect-fill (cropped when capped), radius 12, decoded downsampled to the displayed size off the main
   thread; #E5E5EA until decoded.
2. **thumbs** — `title` 15 semibold. Then a 4 × 3 grid of the 12 `thumbs`: gap 4, square cells of side
   (C − 12)/4, radius 8, aspect-fill. **Skeleton**: each time the row appears (mount, or a recycled
   cell given this row), for 1,200 ms every cell is #E5E5EA with a **shimmer**: one horizontal linear
   gradient band across the whole grid — transparent → rgba(255,255,255,0.65) at its middle →
   transparent — 40 % of the grid width wide, its left edge moving linearly from −40 % to 100 % of the
   grid width in 1,000 ms, repeating, seen only through the cells. Then the images fade in (opacity
   0 → 1, 200 ms).
3. **shader** — `caption`, then the photo in a C × round(C·9/16) box (aspect-fill, radius 12) with a
   live filter. For each pixel at box-local point (x, y), with t = the motion clock in seconds:
   `d = 0.012·W; sx = clamp(x + d·sin(24·y/H + 2t), 0.5, W − 0.5); sy = clamp(y + d·cos(18·x/W + 1.6t), 0.5, H − 0.5);
   c = sample(sx, sy); l = 0.2126 c.r + 0.7152 c.g + 0.0722 c.b; out = mix(c, mix(duoA, duoB, l), 0.7)`,
   on sRGB-encoded values, alpha 1. Redrawn every frame while on screen.
4. **canvas** — one canvas C × 220, radius 12 (clipped), background `bg`, drawn in this order
   (unit coordinates × (W, H)):
   1. a rounded rect at (16, H − 60), 0.4·W × 44, radius 10, filled with a left→right linear
      gradient `band[0]` → `band[1]`;
   2. the 6 `strokes`: a cubic Bézier from (p0,p1) via (p2,p3), (p4,p5) to (p6,p7), `width`, `color`,
      round caps and joins, no fill;
   3. the 14 `dots`: circles at (x·W, y·H), radius `r`, `color` at 60 % opacity;
   4. **an image**: the `image` JPEG 64 × 64 at (W − 80, 16), aspect-fill, clipped to a circle;
   5. **text**: `title`, system 20 semibold #1C1C1E, its box's top-left at (16, 16), one line.
5. **svg** — (a) a row of the 6 `icons` (`icon-<name>.svg`), each 28 × 28, 16 gap, left-aligned;
   (b) `chart` (`<name>.svg`) at C × C·160/600; (c) `art` at C × C/2, radius 12 (clipped). 12 pt gaps.
   Drawn as vectors at display size from the SVG files (the files are the source; no pre-rasterising).
6. **video** — `caption`, then `video` in a C × round(C·9/16) box: aspect-fill, radius 12 (clipped),
   **muted, autoplay, looping, no controls**, and a pill at bottom-left (8 pt inset): `0:04` 12
   semibold white on rgba(0,0,0,0.55), padding 3/7, radius 6. Plays while on screen; off-screen
   behaviour is the platform's (pause, release or recycle).
7. **map** — `place` 15 semibold, `address` 13 #8E8E93 (2 pt gap). Then a map C × 200, radius 12
   (clipped): Apple Maps, standard style, centre (`lat`, `lon`), span 0.02° × 0.02°, one marker at the
   centre titled `place` (the platform's default marker), **all interaction disabled**, live tiles.
8. **markdown** — `md` rendered as blocks, 8 pt apart: `## ` heading 20 semibold; paragraph 16 regular;
   `**bold**` weight 700; `*italic*` italic; `` `code` `` Space Mono 14 on #F2F2F7; `[link](…)` #007AFF,
   no underline; `- ` bullet list: "•" then 8 gap then the item, items 4 apart; `> ` blockquote: 3 pt
   #C7C7CC bar at left, 10 pt padding, 16 italic #3C3C43.
9. **code** — a card: background #0D1117, radius 12, padding 12. Header line: `file` Space Mono 12
   #8B949E left, the language (`TS`, `RUST`, `PYTHON`) 11 semibold #8B949E right; 8 gap. Then one row
   per line, 20 pt pitch: the line number (from 1) Space Mono 13 #6E7681 right-aligned in a 24 pt
   column · 12 gap · the line's tokens, Space Mono 13, **no wrapping** (clipped at the card's padding).
   Token colours by `k`: keyword #FF7B72, string #A5D6FF, number #79C0FF, comment #8B949E,
   function #D2A8FF, type #FFA657, plain #E6EDF3. Highlighting is done in `gen.py` (tokens), since
   neither stack has a built-in highlighter.
10. **intl** — the 6 `blocks`, 8 apart, system 17 regular #000 with the platform's font fallback:
    Arabic and Hebrew (`dir: rtl`: base direction right-to-left, right-aligned), CJK (ja / zh / ko),
    Devanagari (Hindi), an emoji line of ZWJ sequences, skin tones and flags, and a mixed-direction
    line (`dir: ltr`, Latin with an embedded Arabic or Hebrew run and digits). LTR blocks left-aligned.
11. **typeface** — a card, background `bg`, radius 12, padding 20: `quote` in the bundled font
    `fonts[font]` (PostScript `name`, `size`), #1C1C1E, wrapping; 10 gap; `— {by} · {family}` 13
    regular #3C3C43 (system). The font rotates by block: all rows of block b use `fonts` key b mod 10.
12. **carousel** — `title` 17 semibold. Then a **horizontal scroller** C wide, 170 tall, free
    scrolling, no indicators, no paging, starting at offset 0 (a recycled row starts at 0 too): 10 cards
    140 wide, 10 apart; card: `image` 140 × 100 radius 10 aspect-fill; 6 gap; `title` 13 semibold #000,
    at most 2 lines, tail ellipsis, in a 34 pt box; 4 gap; `meta` 12 #8E8E93, one line.
13. **motion** — `caption`, then three tiles side by side, 12 apart, each square of side (C − 24)/3,
    radius 12, background #F2F2F7: the `gif`, the `webp`, the `lottie`, each aspect-fit, looping
    forever at their own timing. Under each tile (4 gap) a centred label `GIF` / `WebP` / `Lottie`
    11 semibold #8E8E93.
14. **glass** — the `photo` in a C × round(C·3/4) box, aspect-fill, radius 16 (clipped), with two
    overlays of a light **ultra-thin material** (blur of what is behind + a light tint): a bar inset 12
    from left, right and bottom, 64 tall, radius 14, holding (padding 12 horizontal, centred
    vertically, 2 gap) `title` 16 semibold #000 and `subtitle` 13 #3C3C43, one line each, tail
    ellipsis; and a capsule at the top-right (12 inset), `rating` 13 semibold #000, padding 6/10.
15. **live** — a card: border 0.5 #D1D1D6, radius 12, padding 14:
    - `title` 15 semibold left; right: `Updated {u}s ago` 12 #8E8E93, u = (`updatedSec` + s) mod 60.
    - 10 gap. `Ends in ` 15 regular #3C3C43 followed by `HH:MM:SS` 22 semibold #000 with tabular digits,
      from max(0, `endsInSec` − s).
    - 12 gap. Three **progress rings**, 56 pt outer diameter (the stroke's centre on radius 25), 16 apart, stroke 6 with round caps over a
      #E5E5EA track, colours #FF3B30, #34C759, #007AFF, the arc from 12 o'clock clockwise to
      p_i = clamp(`rings[i]` + 0.2·sin(2π(t/4 + i/3)), 0, 1) (t = motion clock, redrawn every frame);
      centred label `round(rings[i]·100)%` 12 semibold #000 (static).
    - 12 gap. A **waveform** row: a 32 pt #007AFF circle holding a white right-pointing triangle 10 wide × 12
      tall, centred · 10 gap · 48 bars, 3 wide, 2 apart, heights `wave[j]` pt centred in a 40 pt box, radius 1.5,
      #007AFF where j < 48·q else #C7C7CC, q = (t mod 12)/12 (motion clock) · 10 gap ·
      `0:0X / 0:12` 12 #8E8E93 tabular digits, X = s mod 12 (`0:10`, `0:11` for 10, 11).
16. **thread** — `text` 15 regular #000: collapsed it shows at most 3 lines (tail ellipsis), then 4 gap
    and `Show more` 15 semibold #007AFF; expanded, the full text then `Show less`. Tapping the label
    toggles. The height change animates over 250 ms, ease-in-out. In live mode the row's state follows
    the clock: expanded iff (⌊s/3⌋ + `index`) is odd, re-applied on every tick (so a visible thread row
    flips every 3 s; a tap holds until the next flip). Then 12 gap and a **comment box**: a text field
    (flexes) 40 tall, background #F2F2F7, radius 20, 14 horizontal padding, 15 regular, placeholder
    `Add a comment…` #8E8E93 · 8 gap · `Send` 15 semibold, #007AFF when the draft is non-empty else
    #C7C7CC. The draft is app state keyed by row id (survives scrolling away and back).
17. **webview** — `caption`, then a web view C × 220, radius 12 (clipped), 0.5 #D1D1D6 border, showing
    `embed.html` with `{{HUE}}` = `hue`, `{{TITLE}}` = `title`, `{{N}}` = `index`, `{{PLAY}}` = `running`
    (`paused` under freeze), `{{BARS}}` = 12 × `<div class="bar" style="height:{v}%"></div>` from
    `bars`, loaded as an HTML string with no base URL. The page has a CSS keyframe spinner. Its own
    scrolling is disabled.
18. **filmstrip** — a nested **horizontal virtualized list** of 2,000 thumbnails.
    - `title` 15 semibold #000 (`2,000 photos from …`).
    - Then the strip: C wide × 136 tall, free scrolling, no indicators, no paging, no content inset.
    - Items: `count` = 2,000, each 112 wide, 8 apart, laid out left to right. Content width
      2,000 × 120 − 8 = 239,992 pt.
    - Item j (from 0):
      - The image `small-{(img0 + j·imgStep) mod 48, 2 digits}.jpg`, 112 × 112, radius 8, aspect-fill,
        decoded downsampled to the displayed size, #E5E5EA until decoded. `imgStep` is coprime to 48, so
        every image appears once in each run of 48.
      - 4 gap, then the caption `IMG_{num0 + j}`, 12 regular #3C3C43, one line, left-aligned.
    - **Virtualized.** Only the items near the visible range exist. 2,000 laid-out items per row would
      not be a list.
    - **Keeps its offset.** A strip scrolled to x keeps x when its row scrolls out of the feed and back,
      whether or not the platform recycled the row's view. It is app state keyed by row id, like the
      thread's draft. A strip never scrolled starts at 0.
19. **inbox** — a nested **vertical virtualized list** of 1,000 messages in a fixed box.
    - `title` 15 semibold #000 (`#word-word · 1,000 messages`).
    - Then the box: C × 400, white, 0.5 #D1D1D6 border, radius 12, clipped. It scrolls vertically inside
      the row with the platform's default indicator; no content inset.
    - Items: `count` = 1,000. Message j is `messages[(m0 + j·mStep) mod 997]`. The pool has 997
      entries, a prime, so a row shows all 997 before repeating.
    - Each message is a row with padding 10 vertical, 12 horizontal, holding:
      - The avatar (`avatar`), a 32 × 32 circle, top-aligned.
      - 10 gap, then a column (2 gap):
        - Line 1: `name` 14 semibold #000, one line, tail ellipsis · flexible space (min 8) · the
          time 12 #8E8E93, tabular digits. The time is `HH:MM` (24 h, zero-padded) of
          ((`clock0` − 7·j) mod 1440) minutes.
        - Then `text` 13 regular #3C3C43, at most 2 lines, tail ellipsis.
      - A 0.5 #E5E5EA hairline at the row's bottom, from x = 54 to the right edge, drawn over the row
        (it adds no height).
    - Row height: about 55 pt for one line of text, 71 for two. Pool texts are half one short sentence
      (one line) and half two long sentences (clamped to two). The content is about 66,000 pt.
    - **Virtualized** and **keeps its offset** across recycling, as the filmstrip does.
    - **Nested scrolling.** A drag that starts inside the box scrolls the box. What happens at its edge
      is the platform's default for same-axis nested scroll views, uncustomised (see Decisions). A drag
      that starts outside the box scrolls the feed.

## Clocks, live ticks, freeze

- **Motion clock t** (seconds since launch, continuous): drives the shader, the rings, the waveform
  playhead; videos, GIF/WebP, Lottie, the shimmer and the web view's spinner run on their own timing.
  Motion always runs (whatever `BENCH_LIVE` says) unless frozen: it is what the content does.
- **Live clock s** (whole seconds since launch, a 1 Hz tick): drives `rel`, `Updated …`, the countdown,
  the waveform's time text and the thread flips. It ticks iff `BENCH_LIVE=1` or `BENCH_SCENARIO=rest`,
  and `BENCH_FREEZE≠1`; otherwise s = 0. Read from the launch environment at startup.
- The nested lists have no clocks: freeze changes nothing in them. After a launch, every strip and
  inbox starts at offset 0.
- **`BENCH_FREEZE=1`** (parity screenshots): s = 0; t fixed: the shader at t = 1.25, rings at
  p_i = `rings[i]`, waveform q = 0.4; videos paused on frame 0; GIF/WebP on frame 0; Lottie at progress
  0.5; the thumbs skeleton skipped (images shown); the web view's spinner paused; threads collapsed.
  Map tiles still load over the network.
- **`BENCH_START_INDEX=n`**: row n at the top at launch (for "after a jump" shots).
- The `rest` scenario leaves the list at the top (offset 0).
- Every app supports all four orientations on iPad, so all run in the device's orientation.

## Scenarios

The probe's (`../heavy-list/probe/README.md`): `fling` (±1k–24k pt/s, 2 s segments after a
3k warm-up), `ladder` (±3k–96k), `jump` (10 absolute jumps), `coldstart`, `rest` (no scroll, 10 s,
live ticks on), and **live** = `fling` with `BENCH_LIVE=1`. Runs go through the probe's `devrun.sh` (`ios/series.sh`),
under an optional device lock (`BENCH_LOCK`).

The smoke runs (2026-09-27) were `fling-0`, `rest-0`, `innerfling-0`, `innerfling-4` and
`innerkeep-0` per app.

Two scenarios exercise the nested lists (added to the probe 2026-09-27). The probe recognises an inner
list by shape, not by class:
- a **strip** is a descendant scroll view with content width ≥ 20,000 pt and height < 300 pt;
- an **inbox** is a descendant 300–600 pt tall and narrower than the feed, with content height
  ≥ 20,000 pt.

Neither scenario synthesises touches: the probe sets `contentOffset`, as it does for the feed.

- **`innerfling`.** From the launch offset, the probe scrolls the feed until a strip is fully in view,
  its top a quarter of the way down (row 3 at launch). It waits 0.8 s, then drives the **strip's**
  offset at the ladder speeds: a ±3k warm-up, then ±3k, 6k, 12k, 24k, 48k and 96k pt/s, 2 s each. It
  then does the same inside an inbox (row 7). The inbox's 66,000 pt clamp the travel at 48k and 96k;
  the `travel%` column shows it. With `BENCH_SAMPLE`, blank bands are measured inside the inner list:
  columns ≥ 60 pt wide for the strip, rows ≥ 60 pt tall for the inbox.
- **`innerkeep`.** For each inner kind, the probe finds one and sets its offset to a mark (strip 12,345
  then 23,456; inbox 5,432 then 8,765). It then flings the **feed** away and back, first 3,000 pt at
  6k pt/s ("near"), then 24,000 pt at 12k pt/s ("far": the row is surely recycled). It snaps the feed
  back to the marked offset and waits 0.8 s. It then reads the inner list found at the marked place in
  the window. It records `keep` = `{kind, trip, mark, markRead, got, err, kept (|err| < 1 pt),
  sameView}`. The four feed-fling segments per kind are measured like `fling`'s.

## Parity

Screenshots with `BENCH_FREEZE=1` at the top and at `BENCH_START_INDEX` 3 (a filmstrip), 7 (an inbox),
500 / 1,500 / 2,500,
compared by eye and row geometry (`ios/shoot.sh`). Maps draw live tiles, so their labels may differ by load
state; the platforms' own materials, markers, video scaling and text shaping may differ slightly and
are listed under Decisions.

## Decisions

Resolved by the builder, 2026-09-27; the most ordinary choice for each stack, and where a stack
genuinely cannot do it the same way.

- **Expo: LegendList 3.4.0 (`@legendapp/list`) instead of FlashList, per Charlie 2026-09-27.** Every
  list in the app (the feed, the carousel, the filmstrip, the inbox) is a `LegendList` written as its
  README and performance guide say: `keyExtractor`, `recycleItems`, `getItemType` where rows differ,
  `getFixedItemSize` where item sizes are truly fixed, `estimatedItemSize` otherwise, the default
  `maintainVisibleContentPosition` (scroll-time stabilisation on, data anchoring off: the data never
  changes). The FlashList 2.0.2 build stays measurable as `expo/xheavy-expo-iphoneos-flashlist.zip`.

**The list**

| | SwiftUI | Expo |
|---|---|---|
| list | `List` `.listStyle(.plain)` + `ForEach` over `@State [Row]`, one `switch` on kind | `LegendList` 3.4.0 with `recycleItems`, `getItemType = kind`, `keyExtractor = id`, `estimatedItemSize={470}` (≈ 1.4 M pt / 3,000 rows; LegendList then keeps a measured average per item type) |
| images | `AsyncImage` cannot downsample or read the bundle; the ordinary bundled-image path is `Image(uiImage:)` from an ImageIO thumbnail decoded in a `.task` (the heavy bench's loader) | `expo-image` (`contentFit="cover"`, `recyclingKey`) |
| shader | `.layerEffect(ShaderLibrary.xheavyWave(…), maxSampleOffset:)` (a `[[stitchable]]` Metal function in `default.metallib`) inside `TimelineView(.animation)` | `@shopify/react-native-skia` `Canvas` + `Skia.RuntimeEffect` (SkSL) with an `ImageShader` child (`fit="cover"`), time from Reanimated `useFrameCallback` |
| canvas | SwiftUI `Canvas` (`context.draw(Text)`, `context.draw(Image)` under a circle clip) | Skia `Canvas` (`Path`, `Circle`, `LinearGradient`, `Text` with the system font via `matchFont`, `Image` under `clipPath`) |
| svg | **SwiftUI has no SVG view.** The ordinary equivalent is SVG files in an asset catalog (`Assets.xcassets`, "Preserve Vector Data") drawn with `Image("chart-03").resizable()` — CoreSVG renders them as vectors at any size | `react-native-svg` `SvgXml` with the XML from `svgs.json` |
| video | `AVQueuePlayer` + `AVPlayerLooper` in an `AVPlayerLayer` via `UIViewRepresentable` (SwiftUI's `VideoPlayer` always has controls and cannot aspect-fill) | `expo-video` `useVideoPlayer` (`loop`, `muted`) + `VideoView` (`contentFit="cover"`, `nativeControls={false}`) |
| map | MapKit for SwiftUI `Map(initialPosition:interactionModes: [])` + `Marker` | `react-native-maps` `MapView` (Apple provider) with every `*Enabled={false}`, `pointerEvents="none"`, default `Marker` |
| markdown | block split in the app (`## `, `- `, `> `, paragraphs) and `Text(AttributedString(markdown:))` per block for the inline syntax — SwiftUI renders only inline Markdown | `react-native-markdown-display` with the spec's styles |
| code | `Text` concatenation of coloured runs, `.fixedSize` + clipped | nested `Text` runs, `numberOfLines={1}` without ellipsis (`ellipsizeMode="clip"`) |
| fonts | `UIAppFonts` + `Font.custom(name, fixedSize:)` | `expo-font` config plugin (fonts embedded at build) + `fontFamily: name` |
| carousel | `ScrollView(.horizontal)` + `LazyHStack` | a nested `LegendList horizontal`, `recycleItems`, `getFixedItemSize` 140, `contentContainerStyle={{ gap: 10 }}`, `dataKey = row id` (a recycled row is a new dataset and starts at its first card) |
| GIF / WebP | **SwiftUI has no animated image.** `UIImageView` via `UIViewRepresentable`, frames from ImageIO's `CGAnimateImageAtURLWithBlock` (GIF and WebP) | `expo-image` (animates GIF and WebP) |
| Lottie | `lottie-ios` (`LottieView`, compiled from source into the app: `vendor/lottie-ios` at its commit) | `lottie-react-native` (the same `lottie-ios` underneath) |
| glass | `.background(.ultraThinMaterial)` | `expo-blur` `BlurView tint="systemUltraThinMaterialLight"` (the same UIKit material) |
| live | `TimelineView(.animation)` for t; a 1 Hz `Timer.publish` for s | Reanimated shared value driven by `useFrameCallback` for t (rings/waveform in Skia); `setInterval(1000)` for s |
| height change | `lineLimit(expanded ? nil : 3)` under `withAnimation(.easeInOut(duration: 0.25))` | `numberOfLines` switch + `LayoutAnimation.configureNext(250 ms easeInEaseOut)`; expansion and draft in `useRecyclingState` (LegendList's per-item state, reset when a recycled cell gets another row; its setter re-lays out the cell) seeded from the per-row maps |
| shimmer | a `LinearGradient` band offset by a repeating animation, masked by the cells | `expo-linear-gradient` in an `Animated.View` translated by a looping Reanimated timing |
| web view | `WKWebView` via `UIViewRepresentable`, `loadHTMLString(_, baseURL: nil)`, scroll disabled | `react-native-webview` `source={{ html }}`, `scrollEnabled={false}` |
| text input | `TextField` bound into `@State [String: String]` | `TextInput` controlled by a draft map in app state |
| filmstrip | `ScrollView(.horizontal, showsIndicators: false) { LazyHStack(spacing: 8) { ForEach(0..<2000) … } }` in the `List` row, `.frame(width: C, height: 136)`; images through the same `BundleImage` loader | a nested `LegendList horizontal` over 2,000 items in a C × 136 `View`: `recycleItems`, `keyExtractor`, `getFixedItemSize` 112, `contentContainerStyle={{ gap: 8 }}` (LegendList's own spacing, counted in fixed sizes), `expo-image` with `recyclingKey` |
| inbox | a fixed-height `ScrollView { LazyVStack(spacing: 0) { ForEach(0..<1000) … } }`, `.frame(width: C, height: 400)`, clipped, bordered overlay | a nested vertical `LegendList` (`recycleItems`, `keyExtractor`, `estimatedItemSize={70}`, a two-line message; `nestedScrollEnabled`) in a C × 400 `View` (`overflow: hidden`, border) |
| inner offset kept | `.scrollPosition($position)` (`ScrollPosition(x:)` / `(y:)`) restored `onAppear`, and `.onScrollGeometryChange` writing the offset into a plain (unobserved) map keyed by row id: iOS 18 API behind `#available`; on iOS 17 a strip would start at 0 | `onScroll` writes the offset into a module-level `Map` keyed by row id; the inner list's `dataKey` is the row id and its `initialScrollOffset` is the map's entry, so a recycled cell handed another row starts that row's dataset at its kept offset (LegendList's documented dataset switch) |

**Exact2** (`exact-xheavy/`, built 2026-09-27 against origin/main; `exact-xheavy/README.md` has every kind)

| | Exact2 |
|---|---|
| list | one `list virtualized=true estimated-item-height=470` + keyed `each`; the row is a header and one body per kind, chosen by the row's one `some` kind field (`match r.photo …`) |
| images | `image … object-fit="cover"` (the host's downsampled ImageIO decode off the main thread) |
| shader | a GPU `canvas surface=wave(…)` (LLP 1009): the module asks for the JPEG, decodes it off the main thread to ≤ 1,200 px, samples it through the wave + duotone in WGSL every frame |
| canvas | Canvas 2D (LLP 1056 stage 2) drawn by the Rust data source: `roundRect`, `createLinearGradient`, `bezierCurveTo`, `arc`, `globalAlpha`, `drawImage` under an `arc` clip, `fillText` |
| svg | the SVG files as generated components (SVGR style, `gen-svgs.py`), inline `svg` |
| video | `video autoplay muted loop playsinline controls=false paused=freeze object-fit="cover"` |
| map | the app's copy of exact2's `native-map` module (MKMapView), `interactive="false"` |
| markdown | parsed by the data source into blocks of runs; nested `text` with the spec's styles (`markup="markdown"` fixes its own styles) |
| code | `white-space: pre` runs, clipped by the card |
| fonts | ten declared `font` families, the family chosen per row by a literal ternary |
| carousel | a nested horizontal virtualized list (`display: flex; flex-direction: row`), `scroll-restoration="manual"` so a recycled row starts at its first card |
| GIF / WebP | `image` (animated natively on Apple since LLP 1011.000) |
| Lottie | **declared difference: Lottie is refused by exact2 (rules/DEFERRED.md, upheld by Charlie 2026-09-27).** Each of the four Bodymovin files is ported by hand to inline SVG + CSS `@keyframes` (`exact-xheavy/lottie.contract`): the ball on `cy` (ease-in-out halves), the square on `rotate`, the ring on `scale` + `opacity`, 2 s loops, the files' colours |
| glass | `backgroundMaterial="ultra-thin"` (the system ultra-thin material, as SwiftUI's) |
| live | a 1 Hz `every(1000, …)` for s; rings are `stroke-dashoffset` `@keyframes` (one generated rule per base value: keyframes take literals only) and the playhead a `steps(48)` translate window, all in Core Animation on iOS |
| height change | `line-clamp` switch in a box whose `height` moves between `54px` and `auto` (`interpolate-size: allow-keywords`, `transition: height 250ms ease-in-out`); expansion in the data source keyed by row id |
| shimmer | a `linear-gradient` band on a `translate` `@keyframes` (px: `translate` takes no percentages), `animation-delay` 1.2 s gates |
| web view | `iframe src=` a bundled page per row (`gen-assets.py`; no `srcdoc`) |
| text input | `input value change=` → the data source's draft map keyed by row id |
| filmstrip, inbox | nested virtualized lists (LLP 1070 stage 4): a row list over 2,000 shared indices, a 400 pt vertical list over 1,000; the runner keeps each inner offset across its row's retirement (anchor key + offset, by default); the inbox finds message j in the pool by `filter` (no list indexing in Contract) |
| `BENCH_START_INDEX` | `scrollIntoView("feed", key)` (LLP 1070.000) shortly after launch |
| `BENCH_FREEZE` | GIF / WebP shown as first-frame stills (an animated image cannot be paused, as on the web); keyframes paused at the spec's phase with a negative delay |

**UIKit** (`uikit/`, added 2026-09-29, per Charlie: a fourth competitor written as a strong iOS engineer writes
UIKit for performance; `uikit/README.md` has the build). Fairness rule: the same content and the same work as the
other three; idiomatic UIKit optimizations (reuse, prefetch, off-main decode, layer-backed drawing) are the point.

| | UIKit | equivalence |
|---|---|---|
| list | one `UICollectionView`, `UICollectionViewCompositionalLayout` (one full-width item, `.estimated(470)` height; the rows in header-less sections of 500 so a self-sizing re-solve stays in one section, as in the heavy bench's UIKit app), `UICollectionViewDiffableDataSource<Int, Int>` over the row indices; one cell class and reuse identifier per kind; manual frame layout, self-sizing through `preferredLayoutAttributesFitting` (measured once per row and width) | the same rows, header, 16/16 padding and 0.5 pt full-width separator (a `CALayer` per cell; SwiftUI's is the system hairline) |
| images | the SwiftUI app's loader, verbatim: ImageIO `CGImageSourceCreateThumbnailAtIndex` at the aspect-fill pixel size, decoded immediately, off the main thread, `NSCache` of 300 — run on an `OperationQueue` (cores − 2) with cancellation when a cell is reused, plus `UICollectionViewDataSourcePrefetching` on the feed, the filmstrip and the inbox; #E5E5EA until decoded | same decode size, same cache size. `byPreparingThumbnail` was not used: it needs a `UIImage` first and gives an aspect-fit size; ImageIO's thumbnail is Apple's recommended downsample and is what the other native app does |
| shader | an `MTKView` per shader cell (its own display link, `preferredFramesPerSecond` 120) running the SwiftUI shader's math as a fragment shader over the photo (decoded at the box's pixel size by the same loader, uploaded to an `rgba8Unorm` texture off the main thread; aspect-fill by uv crop). Draws every frame while the cell is displayed, paused otherwise. `default.metallib` precompiled on the mini | same formula on sRGB-encoded values, same resolution, every frame on screen (SwiftUI: `TimelineView(.animation)` + `.layerEffect`) |
| canvas | a `UIView` whose `draw(_:)` issues the spec's five steps in Core Graphics (gradient rounded rect, cubic Béziers with round caps, 60 % dots, the image under a circle clip, the title); drawn when the row is configured and again when its 64 pt image arrives | SwiftUI's `Canvas` also rasterizes on the CPU when its inputs change |
| svg | the same asset catalog (every SVG as a vector imageset, "Preserve Vector Data"), `UIImageView` at display size | identical source and renderer (CoreSVG) |
| video | `AVQueuePlayer` + `AVPlayerLooper` in an `AVPlayerLayer` (`resizeAspectFill`), muted; plays from `willDisplay`, pauses at `didEndDisplaying`; the cell keeps its player and swaps the looper when reused for another clip | the SwiftUI app's `PlayerUIView`, with visibility from the collection view instead of `didMoveToWindow` |
| map | `MKMapView` per map cell (kept across reuse), `setRegion` span 0.02°, one `MKPointAnnotation` titled `place` (the default `MKMarkerAnnotationView` balloon, as SwiftUI's `Marker`), every interaction off | live tiles, same marker as SwiftUI |
| markdown | the SwiftUI app's block split; inline syntax by Foundation's `AttributedString(markdown:, .inlineOnlyPreservingWhitespace)` mapped to UIKit attributes (bold 700, italic, Space Mono 14 on #F2F2F7, links #007AFF); one `UILabel` per block / bullet item, a 3 pt bar view for quotes; the attributed blocks are built once per row and cached | same parser and styles; SwiftUI rebuilds them in `body` |
| code | per line a right-aligned number `UILabel` and a coloured-run `UILabel` (`byClipping`) clipped at the card's 12 pt padding, 20 pt pitch | same runs and clipping |
| intl | one `UILabel` per block, attributed with `baseWritingDirection` rtl/ltr and right/left alignment | UIKit right-aligns wrapped RTL continuation lines (SwiftUI's README records it cannot) |
| fonts | `UIAppFonts` + `UIFont(name:size:)` | same files |
| carousel | a nested horizontal `UICollectionView` (flow layout, 140 × 170 items, 10 apart), `reloadData` + offset 0 on reuse | a recycled row starts at its first card, as specified |
| GIF / WebP | the SwiftUI app's `CGAnimateImageAtURLWithBlock` → `UIImageView`, started at `willDisplay` and stopped at `didEndDisplaying` | same decoder and frame delivery |
| Lottie | `LottieAnimationView` (the same vendored lottie-ios, default rendering engine), `loopMode = .loop`, played while displayed; `LottieAnimation` parsed once per file and cached | SwiftUI's `LottieView` is this view underneath |
| glass | two `UIVisualEffectView(UIBlurEffect(.systemUltraThinMaterial))` (light interface style) over the photo | the material SwiftUI's `.ultraThinMaterial` uses |
| live | a 1 Hz `Timer` (common mode) updates the visible cells' text; the rings are `CAShapeLayer`s with `strokeEnd` `CAKeyframeAnimation`s (480 samples of p_i(t) over its 4 s period, 120 Hz preferred), the playhead a discrete 48-step `CAKeyframeAnimation` on a mask's width over 12 s; both begin at launch so they show t mod period and run in the render server | same curves on the motion clock; SwiftUI evaluates them per frame on the main thread in `TimelineView(.animation)` — CA animation is the idiomatic UIKit way (exact2 also uses CA keyframes) |
| height change | `numberOfLines` 3 ↔ 0, then `invalidateIntrinsicContentSize` + `invalidateLayout` + `layoutIfNeeded` inside `UIView.animate(0.25, .curveEaseInOut)`; live flips on the 1 Hz tick for visible rows, set at configure for others | same 250 ms ease-in-out |
| shimmer | a `CAGradientLayer` band on a repeating 1 s `position.x` `CABasicAnimation`, masked by a `CAShapeLayer` of the 12 rounded cells; started at `willDisplay` (each appearance), 1.2 s later the grid fades in over 200 ms | same timing |
| web view | `WKWebView` per web cell, kept across reuse (`loadHTMLString(_, baseURL: nil)` again when the row changes), scrolling off, bordered rounded box | same page and load path; reuse of the view is UIKit's cell reuse |
| text input | `UITextField` (padding 14 via left/right views), `.editingChanged` → a draft map keyed by row id | same state keying |
| filmstrip | a nested horizontal `UICollectionView` (flow, 112 × 136, 8 apart, no indicator, no inset) over 2,000 items with prefetching | virtualized with reuse (SwiftUI's `LazyHStack` does not reuse) |
| inbox | a nested vertical `UICollectionView` (flow) in a bordered 400 pt box; item heights 55/71 from whether each pool message wraps to two lines, measured once per width off the main thread at launch | virtualized with reuse; exact per-item heights (SwiftUI's `LazyVStack` measures as it goes) |
| inner offset kept | `scrollViewDidScroll` writes the offset into a plain dictionary keyed by row id; `configure` reloads and restores it | works on iOS 17 (SwiftUI's needs iOS 18) |
| focus | the feed and the nested collection views: `allowsFocus = false` and `focusItems(in:)` → none (an iPad with a hardware keyboard otherwise walks every visible cell on each focus update while scrolling; traced in the crypto UIKit app; exact2's host does the same). Taps and the comment fields' first responder are unaffected | exact2 parity |
| `BENCH_START_INDEX` | `scrollToItem(at:, at: .top)` three times 250 ms apart (estimated heights, as the SwiftUI app's `scrollTo`) | same |
| `BENCH_FREEZE` | shader drawn once at t = 1.25, rings at base, playhead q = 0.4, videos paused, GIF/WebP frame 0, Lottie at 0.5, no skeleton, web page `paused` | same as the SwiftUI app |

**`BENCH_KINDS=17`** (all apps, 2026-09-27, per Charlie; the UIKit app too): leaves out the filmstrip and inbox rows (2,250 rows), so the three stacks render the same kinds. Unset: all 3,000 rows.

**Where the stacks cannot match exactly**
- *Markers*: SwiftUI's `Marker` is the balloon (`MKMarkerAnnotationView`); react-native-maps' default
  `Marker` on Apple Maps is a pin. Both are "the platform's default marker"; accepted.
- *Map tiles* need the network: live tiles over the iPad's Wi-Fi, not snapshots — maps in a list are
  the point. Tile loading makes map rows the one non-deterministic element; the probe's blank detector
  sees a loading map as the grid colour, not as a blank band.
- *Canvas text*: Skia draws with a `matchFont` system font; SwiftUI's `Canvas` resolves `Text` itself.
  Glyph positions may differ by a pixel.
- *Shader colour space*: SwiftUI hands a layer effect the layer's colours; Skia's runtime effect works
  in the canvas's colour space. Both are sRGB-encoded on this iPad; minor tone differences accepted.
- *SVG*: CoreSVG (asset catalogs) supports paths, basic shapes and linear gradients, which is all the
  files use; `svgs.json` exists so Expo needs no SVG transformer. No SVG uses `<image>` or text.
- *Markdown*: SwiftUI's split is by the app (a few lines); the Expo renderer is a library. Both give
  the spec's blocks.

**Nested lists (kinds 18, 19)**
- *Why they exist.* They are a deliberate stress test: each forces a virtualized list inside a
  virtualized list's row, 750 times in the feed. It is the ordinary way to build these rows in both
  stacks, and the one exact2 refuses today (below).
- *Virtualization.*
  - **SwiftUI.** `LazyHStack`/`LazyVStack` create item views only as they approach the visible range.
    They do not recycle item views; they keep created ones until the stack itself goes away.
  - **Expo.** LegendList (`recycleItems`) recycles its item views within the inner list. The inner list
    is itself part of a feed cell that the feed recycles by `getItemType`, so a recycled strip is handed
    a new `data` array, a new `dataKey` and that row's `initialScrollOffset`. (Until 2026-09-27 this was
    FlashList 2.0.2 with a `scrollToOffset` layout effect.)
- *Offset keeping is app code in both stacks.* Neither platform keeps a nested scroll offset for a row
  that left the screen and came back.
  - **SwiftUI.** The `List` rebuilds a row's views when it is re-realised. On the iPad, `innerkeep` read
    `sameView` = false even after a 3,000 pt trip: the restored offset is the app's map at work.
  - **Expo.** The feed may give the recycled cell another row. With LegendList, `innerkeep` got the same
    `UIScrollView` back for the strip both times and for the inbox after the near trip, another one after
    the far trip (`sameView` false): there the map, through `dataKey` + `initialScrollOffset`, restored it.
    The strip's marks came back exact. The inbox's came back 44 and 53 pt short: LegendList's default
    `maintainVisibleContentPosition` moves the offset to keep the visible messages still while rows above
    are measured against the estimate, and the map records the moved offset. With it off on the inbox
    (a diagnostic build, not the benchmark's) all four marks came back exact. (FlashList 2.0.2 kept all
    four, with the same `UIScrollView` each time.)
  - Each keeps a map from row id to offset, as with the thread draft: SwiftUI with `ScrollPosition` +
    `onScrollGeometryChange` (iOS 18+), Expo with `onScroll` + LegendList's `dataKey`/`initialScrollOffset`. The map is not
    observed state, so recording an offset redraws nothing.
- *Nested scroll gestures.* Both inner lists are `UIScrollView`s inside the feed's `UIScrollView`
  (SwiftUI `HostingScrollView`, RN `RCTEnhancedScrollView`), and neither stack customises gesture
  handling, so both get UIKit's default:
  - The innermost scroll view whose pan recognises first owns the drag for its whole duration. A drag
    that starts in the inbox scrolls the inbox.
  - At the inbox's end, UIKit **rubber-bands the inbox; it does not chain the rest of the drag to the
    feed**. The next drag, started at the edge, again scrolls (and bounces) the inbox. To move the feed,
    the user drags outside the box.
  - A vertical drag on the strip moves the feed: a horizontal-only scroll view's pan does not begin
    for a mostly vertical drag.
  - exact2's iOS host documents the same UIKit fact (`NodeViewIOS.swift:30-34`: "UIKit does not chain
    a pan out of a nested scroll view at its edge").
  - This is "what each platform normally does" on iOS. It is not the web's behaviour: the web's
    `overscroll-behavior: auto` chains the remaining scroll to the ancestor once the inner box is at its
    edge. Android chains through nested scrolling (`nestedScrollEnabled`, a no-op prop on iOS, which RN
    documents as Android-only).
  - The probe drives offsets directly and cannot test gestures, since this Mac cannot post touch events
    to the iPad. The behaviour above is UIKit's documented default and was not measured.
- *exact2 has neither piece, and a separate LLP will design it.*
  - A virtualized list inside another list's row template is refused at compile time
    (`contract/lower/src/collection.rs:72-74`, `lower-collection-nested`, "until bounded ancestor-row
    lifetime is supported").
  - A virtualized container cannot be horizontal: `flex-direction`, `flex-wrap` and grid templates are
    refused on it, and `overflow-x` must be `hidden` (`:119-131`).
  - The fallbacks are an eager `scroll` holding 2,000 or 1,000 laid-out children per row, or paging the
    data. `EXACT2-GAPS.md` has the evidence and costs.

**Other**
- Aspect-fill everywhere is the CSS `object-fit: cover`.
- A thread row that is recycled keeps its expansion keyed by row id (app state, like the draft).
- Rows are not given fixed heights: every kind sizes to its content (the thread and thumbs rows
  change height or content after appearing).
- Release builds, iOS 17 deployment target, light mode, landscape iPad at 120 Hz
  (`CADisableMinimumFrameDurationOnPhone`).

## Pre-main inflation in launch numbers (found 2026-09-29, fixed in the probe at 20:1x)

From about 04:00 on 2026-09-29 until the fix, the probe's constructor registered an image-load callback
(`installMetalHook` -> `_dyld_register_func_for_add_image(scanImage)`) that realized every Objective-C class of every
loaded image to find CAMetalLayer subclasses. That ran before `main`, cost ~317 ms on exact2 (App Launch trace:
"Run static initializer probe.dylib"), and scales with the frameworks an app links, so it penalized exact2
(AVFoundation, PhotosUI, AuthenticationServices, MPS, CoreImage, Network) far more than the SwiftUI apps.
**Every cold-start and first-ink number taken with a probe built in that window is insertion-inflated,
and is not comparable across apps.** Scroll metrics (fling, ladder, jump, rest, innerfling, the measured
segments of `still`) start 5–15 s after launch and are not affected by the pre-main cost; the realized classes
add a little resident memory to every app (not re-measured). The fix drops the scan; a CAMetalLayer subclass
that overrides -nextDrawable is hooked lazily when the sampler's layer walk (at most once a second) meets it.
Loading the probe by a load command instead of DYLD_INSERT_LIBRARIES (`mkll.sh`, `devrun-ll.sh`) was tested
and changes nothing (launch-check: exact2 342 vs 340 ms, SwiftUI 205 vs 208 ms), so the inserted probe stays.
