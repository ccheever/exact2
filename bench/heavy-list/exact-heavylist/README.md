# Heavy list — exact2

The heavy list benchmark (`../SPEC.md`) on exact2, built from the checkout this directory is
in. Bundle id `dev.exact.heavybench.exact`. First ported at 9e06c4ea (2026-09-25); this copy
was last built on 2026-10-04 (for the Dioxus comparison; the device series in `../README.md`
ran an earlier copy whose Contract still had `writes` clauses).

- `app.contract` is the whole screen.
- `data/` is the Rust DataSource: the feed, reaction taps, and live mode.
  `data/messages.json` is `../data/messages.json`, copied by `../prepare.sh` (not committed).
  The build refuses compiler inputs outside the app directory, so the file is copied in. It is
  `include_bytes!`'d.
- `assets/` holds the 104 JPEGs from `../data/images/`, copied the same way.
- `apple/` is the static library, whose `build.rs` compiles and bakes the plan.
- `web/` is the web build's crate (`host/web/build.mjs exact-heavylist-web`), which `../../dioxus` measures
  against Dioxus Web on Chrome.
- `Cargo.toml` is its own workspace (so the repository's root workspace does not adopt it),
  with exact2's `[patch.crates-io]` lines. Its `Cargo.lock` is not committed: `../prepare.sh`
  copies the root's, which build.mjs then resolves for this workspace (offline).

## Build and run

From the repository root, after `../prepare.sh`:

```sh
# simulator (cargo release + swift -c release; this is the release build)
EXACT_APP_DIR=$PWD/bench/heavy-list/exact-heavylist EXACT_SIM=<udid> bun host/apple/build.mjs --ios exact-heavylist-apple --run
# drive it / screenshot
EXACT_APP_DIR=$PWD/bench/heavy-list/exact-heavylist EXACT_SIM=<udid> bun scripts/agent.mjs ios "tap messages wheel 0 78900" "clock settle" "screenshot out.png"
# live mode: simctl forwards SIMCTL_CHILD_* to the app's environment
SIMCTL_CHILD_BENCH_LIVE=1 xcrun simctl launch <udid> dev.exact.heavybench.exact
# device: an unsigned archive, unpacked and signed with the probe into build/HeavyExact2.app
bench/heavy-list/exact-heavylist/build.sh
```

build.mjs has no separate debug configuration for apps without `[profile.apple-dev]`: cargo
builds `release` (thin LTO) and Swift builds `-c release`.

## How each spec item is expressed

| Spec item | exact2 feature |
|---|---|
| Data read at startup | `resource initial = messages()`. The bake (`contract::bake` in `apple/build.rs`) compiles the 10,000 rows into the plan (7.8 MB), and the plan decode is the startup read. The DataSource's own editable copy is parsed with serde_json on a background thread at launch and turned into row values (~25 ms on an M-series Mac) on the first `config()`, tap or tick. |
| One list, oldest first | `list virtualized=true estimated-item-height=160` + `each m in feed.rows key=m.id` |
| Top bar | a `row` above the list: "Heavy list" and "Live: on/off" |
| Hairline inset 68 | `box height=0.5 margin-left=68 background-color="#E5E5EA"` at the bottom of each row |
| Avatar 40×40 circle | `image` 40×40, `border-radius=20`, `object-fit="cover"`, `#E5E5EA` background as the placeholder |
| Header line | author 15/600 and `ago(...)` 13 #8E8E93 in a baseline-aligned row; single line by CSS: the author is `white-space="nowrap" overflow="hidden" text-overflow="ellipsis"` (shrinks, ellipsizes), the time `nowrap` and `flex-shrink=0` |
| Relative time | `fn ago(minutes)` using `floor`. `state secs` is set to `floor(performanceNow()/1000)` by `task clock mount` / `every(1000, second)`, only in live mode, so time stays static otherwise. |
| Quote | one `column`: `border-left-style="solid" border-left-width=3 border-left-color="#C7C7CC"`, radius 8, #F2F2F7, padding 8; excerpt `line-clamp=2 text-overflow="ellipsis"` |
| Paragraph = one text flow | one `text` per paragraph whose inline `text` children are the runs (the Markdown app's pattern); `column gap=8` between paragraphs. `white-space` stays `normal`: no run in `messages.json` contains a newline, a tab or a double space, so CSS collapsing changes nothing that SwiftUI would show. |
| bold / italic / link / mention / tag / code | one inline `text` per run: `font-family=(r.s == "code" ? "ui-monospace" : "system-ui")` (a choice of literal families, LLP 1053 G7), `font-size` 15/16, `background-color` #F2F2F7 for code (paints on iOS now), `font-weight`, `font-style`, `color`, `text-decoration-line="underline"` for links (paints on iOS) |
| Photos 1/2/3/4 | `image … object-fit="cover"` in a `border-radius=12 overflow="hidden"` column, sized by CSS layout: one photo is `width="100%" aspect-ratio=<w/h> max-height=320` (aspect-fill within the cap); two and four are `flex=1 min-width=0 aspect-ratio=1` squares in `gap=4` rows; three is a `flex-grow=2 flex-basis=0 aspect-ratio=1` square beside a `flex-grow=1` column that stretches to its height and splits it between two `flex-grow=1` images. Decoding is exact2's raster loader, on a worker at the size the core requests (`CGImageSourceCreateThumbnailAtIndex` with max pixel), with the `#E5E5EA` background until it is ready. |
| Link card | a bordered (0.5 #D1D1D6), radius-12 clipped column; a `width="100%" height=140` cover thumbnail (fixed height, as SPEC says); site / title (`line-clamp=2`) / description (`line-clamp=2`) |
| Reactions | `row flex-wrap="wrap" gap=6` of `button`s (height 28, radius 14, padding 0 10, gap 4) |
| Tap increments | `press=react(m.id, r.emoji)` → `send changed = react(id, emoji)`. The DataSource adds one and answers the feed (like the easy app's bookmark). Verified: 25 → 27 after two taps. |
| Live mode | `task producer mount` / `every(250, tick)`. The first tick sends `config()`, which reads `BENCH_LIVE` with `std::env::var` on the host. After that, in live mode, each tick sends `tick()`: the DataSource puts `live-k` (a copy of `(k*37)%10000`, minutesAgo 0) at the top and bumps a reaction on `(k*101)%10000`, or the next message that has reactions, then answers the whole feed. The answer is a fresh list whose items are the previous answer's `Rc` records for every unchanged message (LLP 1053 §0 G8); a tick builds two records, the insert (sharing its source row's fields but id and time) and the bumped row (sharing every field but `reactions`). The unit test asserts exactly one of the old rows changes identity per tick. |
| Stable viewport on insert | the runner keeps the first visible key and its offset (LLP 1010 §6.2). Verified: 19 inserts over 5 s leave the viewport pixel-identical. |
| Row accessibility label | `aria-label=`${m.author}, ${ago(...)}`` on the row column |

## Gaps

Re-checked against origin/main 9836f237 (LLP 1053's first tranche and the iOS text
fixes).

### Now real (were workarounds)

- **Photo sizes** come from CSS layout (`aspect-ratio`, flex factors, `max-height`), not
  from `exactViewport().width - 84`. The `exactViewport()` resource and the `column`
  prop are gone. Checked: a portrait single photo is 318×320 (capped), a landscape one
  318×178.9, 2/4-photo squares 157×157, the 3-photo layout matches SPEC.
- **Quote bar** is `border-left-color` / `border-left-width` on the quote box.
- **Single-line header**: `white-space: nowrap` + `overflow: hidden` + `text-overflow:
  ellipsis`, no `line-clamp=1`. (The data's names are ≤ 6 characters, so the ellipsis is
  never visible in this benchmark.)
- **Code runs** pick their family with an expression, one `text` per run instead of a
  `when` branch; their #F2F2F7 background paints on iOS (was gap 1).
- **Link underline** paints on iOS.
- **`line-clamp` blank text** (was gap 2) and **mixed-script clipping** (was gap 8) no
  longer reproduce with the old repro scroll; link-card titles/descriptions and m478's
  Arabic line draw.

### Still workarounds or open

1. **No `text-transform`** (LLP 1053 G6, deferred). The link card's site is upper-cased
   in the DataSource.
2. **Live label and first tick** (G9, deferred): `BENCH_LIVE` is read by a `config()`
   mutation on the first 250 ms tick, so "Live: on" appears after ~250 ms.
3. **`background-color="transparent"` is refused** (`lower-attr-value`: "a color is
   `#rgb`, …"). The non-code runs use `#00000000`. CSS accepts `transparent`.
4. **Every reaction tap and live tick re-sends the whole feed** (10,000+ rows), with
   unchanged records shared by `Rc`. Measured on the simulator (`sample`, 1 ms, 15 s of
   live mode, 60 ticks): **~14–15 ms of main-thread runner work per 250 ms tick**, the
   same before and after record sharing (the source's own query is ~0.1 ms). Where it
   goes:
   - ~7.5 ms in `Collection::update_data`: a live insert changes the list length, and
     the collection only reuses keys when the length is unchanged
     (`runner/src/instance/collection/mod.rs:277`, `compare_previous`), so all 10,000+
     keys are evaluated and rebuilt into `BTreeSet`/`BTreeMap<String,_>`
     (`HeightIndex::replace_keys`) every tick, identity or not;
   - ~5.5–6.5 ms in `Value::conforms`, which walks the whole answer (every run of every
     paragraph) twice per tick (`run_action` and `settle_pass`) with no shortcut for a
     shared `Rc` it has already checked.
   Record sharing does help where the length is unchanged (a reaction tap after the first
   one: `compare::same` skips re-keying) and for the mounted rows' dirty checks. The
   first answer after launch cannot share with the baked `initial`, whose records are
   the plan's.
5. **Quote excerpt ellipsis** breaks at a word before the ellipsis ("thanks…"), where
   SwiftUI cuts inside the word ("defi…"). Minor, visible in `exact2-v2-index476.png`.
