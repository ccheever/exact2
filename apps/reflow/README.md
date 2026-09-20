# Reflow

Seven studies after Cheng Lou's Pretext demos, on exact2: one Contract, one Rust
data crate, Linux, macOS, iOS and web. Original prose, set in Exposure Sans
Original 03 (three faces copied from `apps/typetour/assets`; MIT in the Expose
repository).

Pretext has two halves and this app shows both without the library.

- **Text around shapes** is the kernel's (`wrap-flow` / `shape-outside`,
  LLP 1043.000): the balls, the dragon, the drop caps, the pull quote and the
  picture are exclusions every host flows text around.
- **Heights before layout** are the data crate's. `data/build.rs` reads every
  glyph advance and `kern` pair the prose can use out of the app's own font
  files at build time; `data/src/typeset.rs` breaks lines by arithmetic over
  those advances with the same `exact-textflow` walker the hosts flow text
  with. Cards, columns and ASCII rows are measured there, and the kernel
  receives text already cut to the lines the arithmetic found. No font is
  parsed at runtime; the wasm carries a table, not a font.

| Tab / stable id | What to try |
| --- | --- |
| 01 Balls — `scene-balls` (opens first) | Three spheres (`ball-1..3`) cross `balls-prose`; the words part on both sides. Positions are arithmetic on the clock, committed every 16 ms. |
| 02 Dragon — `scene-dragon` | Drag `dragon` anywhere on the manuscript page (`pan`). One polygon is both the ink and the exclusion. |
| 03 Masonry — `scene-masonry` | 36 cards measured in the bold and regular faces, each placed in the shortest column. The footer of every card says the height the arithmetic predicted; `shuffle` reseeds. |
| 04 Dynamic layout — `scene-magazine` | Drag `magazine-handle` at the page's right edge. Columns are re-cut at line boundaries on every sample. |
| 05 ASCII art — `scene-ascii` | A lit sphere in a ring, in a proportional face: each row advances by the kerned width of the glyph it chose. `ascii-naive` samples on a uniform grid instead and the picture bends. |
| 06 Occlusion — `scene-wall` | 1,200 cards, every position known up front; only the cards near the viewport (`wall-N`) exist as views. Scroll `wall-scroll`. |
| 07 Spread — `scene-spread` | An editorial spread whose drop cap, pull quote and picture are exclusions; each column's text (`spread-0..2`) was cut where the same walker, run in the data crate around the same shapes, said the flow would stop. One column on a phone. |

```sh
export EXACT_UPDATE_TRUST=development

# Web (open the URL printed by the dev server)
bun host/web/dev.mjs --app reflow --loopback

# macOS; append --ios for the simulator
bun host/apple/build.mjs --app reflow --run
bun host/apple/build.mjs --app reflow --ios --run

# Linux's CPU painter, headless on macOS too
cargo build --release -p reflow-linux
bun scripts/agent.mjs linux --app reflow --size 1000x900 tree

# The agreement tests: cosmic-text's layout of every card, column and row
# against what the data crate measured.
cargo test -p reflow-data -p reflow-linux --release
```

Drive it (the same lines with `macos`, `web` or `ios` in the host position):

```sh
bun scripts/agent.mjs linux --app reflow --size 1000x900 \
  "clock +1200" "layout balls-prose" "screenshot balls.png" \
  "tap scene-dragon" "tap dragon down" "tap move by -160 120 over 200" "tap up" \
  "layout dragon-prose" "screenshot dragon.png" \
  "tap scene-masonry" "screenshot masonry.png" \
  "tap scene-magazine" "tap magazine-handle down" "tap move by -300 0 over 300" "tap up" \
  "screenshot magazine.png" \
  "tap scene-ascii" "clock +2000" "screenshot ascii.png" "tap ascii-naive" "screenshot naive.png" \
  "tap scene-wall" state "screenshot wall.png" \
  "tap scene-spread" "layout spread-1" "screenshot spread.png"

bun scripts/agent.mjs linux --app reflow --test apps/reflow/app.test.contract
```

Where the arithmetic can disagree with a host: advances are summed per glyph
with `kern` pairs, so a ligature or a shaping rule a face applies across a
segment boundary is a fraction of a pixel the table does not know. Text is
therefore set 2 px wider than it was measured (the card padding) so a
disagreement can only leave air at the bottom of a card or column, never clip
a word. Verified 2026-09-20 on this Mac: the Linux tests above pass with exact
equality (36 card heights, three column line counts, two flowed columns with no
clipped band); on macOS the same scenes match the screenshots' predictions and
the spread's columns end inside their cut; on the web (headless Chrome) flowed
text goes through the shared walker over canvas advances and the spread's two
columns end at bands 45 and 44 of 46, while ordinary paragraphs are broken by
the browser and a magazine column can end one line early. The macOS binary was
ad-hoc signed (`EXACT_IDENTITY=-`) because the keychain's team identity refused
codesign from a non-interactive shell (`errSecInternalComponent`). iOS ran on the
iPhone 17 simulator (balls, spread at one column, ASCII), driven by the same
agent lines.
