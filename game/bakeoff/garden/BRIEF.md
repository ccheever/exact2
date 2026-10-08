# The garden bakeoff — the brief

Every lane receives this file unchanged, plus `ART.md`, the art kit (`art/`), the
reference screenshots (`reference/`) and a lane note (`LANE.md`) that names the engine
and where it is. Nothing else about the game is given to anyone.

You are building **Grow a Garden**, a small 3D farming game, in the engine your lane
note names. A reference version already exists; its screenshots are in `reference/`
and your build should look and play like it. Requirement ids (`G1`, `R4`, `P7`…) are
what the fidelity judge checks, one by one.

Work alone, without asking questions: where this brief is silent, choose what the
reference screenshots suggest and write the choice in your diary.

## 1. The world

- **G1 Field.** A square garden of 2 m tiles, 6 × 6 at the start. Tile `(x, z)`
  (both from 0) is centred at world `(2x, 0, −2z)`: the garden grows east (+x) and
  north (−z) from tile `(0, 0)`. Soil-brown ground with visible tile lines.
- **G2 Surroundings.** The garden sits in a green meadow (`meadow`, `grass-*`,
  `tuft-*`, `path-*`), ringed by a picket fence (`fence-post`, `fence-rail`) with
  lanterns (`lantern`) at intervals, a row of trees beyond the fence (the grown
  `plant-apple-4`, `plant-mango-4`, `plant-coconut-4` models scaled up 2–3×), and a
  seed stall (`stall`) with its keeper (`keeper-*`) beside the garden. The fence
  moves out when the garden expands.
- **G3 Barrel.** A blue water barrel (`barrel`) at world `(−2, 0, 0)`, just west of
  tile `(0, 0)`.
- **G4 Gardener.** A straw-hatted gardener assembled from `farmer-body`, two
  `farmer-arm` and two `farmer-leg` (see `ART.md` for the joints). Starts on tile
  `(0, 0)`. Walks with **WASD** or the arrow keys along the world axes, top speed
  4 m/s, accelerating smoothly; faces the way it walks; arms and legs swing while
  walking. Space hops (optional). The gardener cannot leave a bounded area of about
  20 m around the garden.
- **G5 Camera.** Third person, from the south and above, following the gardener
  smoothly, framed like `reference/arrival.png`. An **overview** toggle pulls back to
  show the whole garden.
- **G6 Plants.** Each crop has five growth-stage models (`plant-<crop>-0` … `-4`).
  A plant shows its current stage's model. Fruit models (`fruit-<crop>-unripe`,
  `fruit-<crop>`) hang on a mature plant; a fruit's body material is recoloured by
  its mutation (§3). Crops stand slightly to one side of the tile centre so the
  gardener does not hide them.
- **G7 Tile outline.** The tile under the gardener is outlined: cyan when empty,
  amber while growing or regrowing, blue after watering, purple after feeding, green
  when fruit is ripe.
- **G8 Day and night.** A ten-minute day: the sky and light move from day through
  dusk to night and back; the lanterns' glass (material 0) glows at night.
- **G9 Weather visuals.** Rain shows falling rain, Snow falling snow, a
  Thunderstorm rain and flashes.
- **G10 Feedback.** Planting kicks up dirt; watering tips a watering can (`can`) and
  drops blue water; harvesting sends the fruit to the gardener with a short sound.
  These are presentation only and must not change any rule or number.

## 2. Crops

Fourteen seeds, cheapest first. Times are seconds of game time; prices and values in
sheckles (¢).

| id | name | rarity | price | grow_s | fruit_s | slots | regrows | value | weight kg | appear | max stock |
|---|---|---|---:|---:|---:|---:|---|---:|---:|---:|---:|
| carrot | Carrot | Common | 10 | 20 | 0 | 1 | no | 18 | 0.25 | 1.0 | 25 |
| strawberry | Strawberry | Common | 50 | 40 | 30 | 4 | yes | 14 | 0.25 | 0.9 | 12 |
| blueberry | Blueberry | Uncommon | 400 | 60 | 40 | 5 | yes | 40 | 0.18 | 0.7 | 8 |
| tomato | Tomato | Rare | 800 | 90 | 60 | 4 | yes | 60 | 0.45 | 0.5 | 6 |
| corn | Corn | Rare | 1300 | 120 | 80 | 3 | yes | 75 | 0.6 | 0.4 | 5 |
| watermelon | Watermelon | Rare | 2500 | 180 | 0 | 1 | no | 2700 | 7.0 | 0.3 | 4 |
| pumpkin | Pumpkin | Legendary | 3000 | 240 | 0 | 1 | no | 3400 | 6.0 | 0.25 | 3 |
| apple | Apple | Legendary | 3250 | 300 | 90 | 6 | yes | 270 | 2.8 | 0.2 | 3 |
| bamboo | Bamboo | Legendary | 4000 | 200 | 0 | 1 | no | 4000 | 3.8 | 0.2 | 5 |
| coconut | Coconut | Mythical | 6000 | 420 | 120 | 3 | yes | 400 | 14.0 | 0.12 | 2 |
| cactus | Cactus | Mythical | 15000 | 360 | 150 | 3 | yes | 3400 | 7.0 | 0.1 | 2 |
| dragon | Dragon Fruit | Mythical | 50000 | 600 | 180 | 4 | yes | 4750 | 11.0 | 0.06 | 2 |
| mango | Mango | Mythical | 100000 | 720 | 200 | 4 | yes | 6300 | 14.0 | 0.04 | 1 |
| grape | Grape | Divine | 850000 | 900 | 240 | 6 | yes | 7850 | 3.0 | 0.01 | 1 |

Rarity colours: Common `#9ca3af`, Uncommon `#4ade80`, Rare `#60a5fa`, Legendary
`#facc15`, Mythical `#c084fc`, Divine `#fb923c`.

## 3. Rules

- **R1 Growth.** A planted seed is stage 0 and reaches stages 1, 2, 3, 4 at ¼, ½, ¾
  and all of `grow_s`. At stage 4 it is mature and sets `slots` fruit.
- **R2 Fruit.** A regrowing crop's fruit ripens `fruit_s` after it is set; harvesting
  it sets a new one that ripens `fruit_s` later. A single-harvest crop (`fruit_s` 0)
  is ripe at maturity, and harvesting it removes the plant and frees the tile.
- **R3 Ripening roll.** When a fruit ripens it rolls, in this order: a growth variant
  (Rainbow with probability 0.001, otherwise Gold with 0.01); the weather's mutations
  (Rain: Wet 0.5. Snow: Chilled 0.4, otherwise Frozen 0.1. Thunderstorm: Wet 0.5,
  and independently Shocked 0.03); a weight of `base weight × uniform(0.8, 1.4)`,
  tripled with probability 0.02; and ×1.25 if the fruit was fed (R9).
- **R4 Value.** `value = round(base value × (weight / base weight)² × variant × (1 +
  Σ(environment multiplier − 1)))`, at least 1. Variant: Rainbow 50, Gold 20, none 1.
  Environment multipliers: Wet 2, Chilled 2, Frozen 10, Shocked 100. A fruit's name
  lists them: "Fed Gold Wet Carrot". Worked values: a 0.25 kg carrot is 18¢, 0.5 kg
  72¢, Gold 360¢, Gold Wet 720¢, Rainbow Wet Chilled 2700¢.
- **R5 Weather.** The garden starts Clear. The first change comes after a uniform
  180–420 s. From Clear the next sky is Rain (0.6), Snow (0.25) or Thunderstorm
  (0.15), lasting a uniform 60–120 s; after it, Clear again for 180–420 s.
- **R6 Purse and start.** Start with 20¢ and one Carrot seed in hand.
- **R7 Planting and harvesting.** **E** on an empty tile plants the held seed (one is
  used). **E** on a plant with ripe fruit harvests all of its ripe fruit into the
  backpack. Otherwise E does nothing but the prompt says why.
- **R8 Watering.** **Q** waters the plant on the current tile: the remaining wait to
  maturity (or, when mature, to its unripe fruit's ripening) shrinks by 25%; the
  remaining stages keep their proportions (planted at 0 s, grow_s 20, watered at
  4 s: mature at 16 s).
  The can holds 3 doses. A growing plant takes one dose per growth; when mature, its
  unripe fruit can take one; each regrowth once. Watering a ripe plant, an empty tile
  or a plant already watered for this growth spends nothing. **R** refills the can for
  free within 2.5 m of the barrel.
- **R9 Compost and feed.** Composting a chosen backpack fruit removes it and adds one
  dose of plant food (pouch holds 3; a full pouch leaves the fruit). **F** feeds the
  plant on this tile: its upcoming fruit weighs 25% more (R3). Feeding a seedling
  carries to its first fruit; repeated feeding of the same growth spends nothing.
- **R10 Shop.** Restocks every 300 s of game time and once at start: each seed
  appears with its `appear` probability, with a uniform stock of 1 to `max stock`.
  The seed for the current market order always has at least 1. Buying takes the price
  and one stock and puts the seed in hand (holding `×N`).
- **R11 Backpack and selling.** Harvested fruit lists its name, weight and value.
  Sell one, or sell all.
- **R12 Market orders.** Five orders, in order: 1 Carrot (+30¢ bonus), 4 Strawberry
  (+400), 5 Blueberry (+600), 4 Tomato (+1300), 3 Corn (+2400). **Deliver order**
  takes the requested fruit from the backpack and pays their full value plus the
  bonus. After the fifth: "all 5 orders filled".
- **R13 Expand.** Expanding grows the garden from size `n × n` to `(n+1) × (n+1)`
  for `n³ × 25`¢, up to 256.
- **R14 Away.** The game saves. Re-opening a save later grows the garden by the real
  time that passed while it was closed: every growth, ripening, restock and weather
  change that would have happened happens, in order, with the same random draws as
  playing through.
- **R15 Determinism.** One seeded random stream drives every roll. The same seed and
  the same inputs give the same game.

## 4. The interface

Real UI (text, buttons, keyboard focus, accessible names), not pictures of UI. Lay it
out like `reference/grown.png` and `reference/market.png`.

- **U1 Title.** The game's name and a **Play** button.
- **U2 Top bar.** Purse (`20¢`, compact above 1,000: `3K¢`, `1.2K¢`, `3.4M¢`),
  restock countdown (`Restock 2:30`), a garden summary (`100 plants · 75/124 ripe · 0
  mutated`), the weather when not clear, the day clock; and **Shop**, **Backpack N**,
  **Garden**, **Pause** buttons.
- **U3 Shop panel.** Every seed with rarity (coloured), price, stock, `×N` held,
  time to first fruit and regrowth time, and **Buy** (disabled when unaffordable or
  sold out).
- **U4 Backpack panel.** Fruit rows (name with mutations, weight, value) with
  **Sell** and **Compost** per row, **Sell all** with the total; it stays responsive
  with thousands of fruit.
- **U5 Garden panel.** **Harvest all**, **Expand** (with its cost), **Overview**
  toggle; stress tools **+100 plants** (free random seeds on empty tiles, expanding as
  needed) and **Skip 1 hour** (advance game time an hour, as R14).
- **U6 Market panel.** The current order, how many are in the backpack, the bonus,
  the next step, and **Deliver order**.
- **U7 Care panel.** Watering can `N/3` with **Q Water** and **R Refill**, plant food
  `N/3` with **F Feed**, and a line saying what each would do here.
- **U8 Prompts.** Under the gardener: the plot (`Plot 1, 1 · Carrot`), what E does
  here (`E: plant Carrot`, `E: harvest 1 Carrot`, `Carrot growing · 0:12`), a hint
  toward the nearest empty plot or back to the garden (direction as a WASD key and
  metres), and a short note after each action (`Planted Carrot`, `Sold for 18¢`).
- **U9 Pause.** Stops game time; the button reads Resume.

## 5. The proof (what "done" means)

An automated script, run from the command line with no human watching, with the
game's clock under the script's control (no real-time waiting for growth), that:

- **P1** starts a new game with a fixed seed and asserts 20¢, one Carrot seed in hand,
  a 6 × 6 garden and a shop with Carrot in stock;
- **P2** walks the gardener with the movement keys and asserts it moved, and that
  movement for a fixed time is the same in two runs;
- **P3** plants the Carrot on tile `(0, 0)` with E and asserts stage 0 and an empty
  hand; advances 10 s and asserts stage 2; advances to 20 s and asserts a ripe fruit;
- **P4** harvests it with E and asserts the backpack holds one Carrot whose value
  obeys R4 for its rolled weight and mutations;
- **P5** delivers market order 1 and asserts the purse rose by value + 30¢;
- **P6** buys a Carrot, plants it, waters it at 4 s and asserts it ripens at 16 s
  instead of 20 s, the can at 2/3, and a refill at the barrel restoring 3/3;
- **P7** saves mid-game, restores in a **fresh process**, advances both by the same
  span and asserts identical state (a canonical dump of every saved field);
- **P8** restores the same save with the clock one hour later and asserts the
  offline growth (R14) equals playing that hour through;
- **P9** plants 100 with the stress tool, skips an hour, and reports the frame time
  with ~100 and ~1,000 plants;
- **P10** writes screenshots of the running game: `title.png`, `arrival.png`
  (the start, framed like `reference/arrival.png`), `planted.png`, `grown.png`
  (after +100 and an hour), `shop.png`, `backpack.png`, `market.png`, `night.png`,
  `rain.png`, into `artifacts/`.

If the engine cannot do one of these, the diary says which and why, and what the
closest honest substitute was. One command runs the whole proof; put it in
`README.md`.

## 6. The diary (keep it as you go)

`DIARY.md`, appended as you work, never rewritten at the end:

- A timestamped line (UTC, from `date -u`) at the start, at every build/run of the
  proof, at every place the engine or its tools fought you, and at the end.
- The friction log: one line per item — what was clumsy, what it cost (minutes,
  rebuilds), how you got past it.
- At the end: lines of game code (excluding the engine, art and generated files),
  files written, build/run iterations to a passing proof, how many times you looked
  at pixels, and which brief items are not done or only approximated.

A diary that flatters is worthless; friction is the finding.

## 7. When you are done

Write `DONE.json` in your lane directory:

```json
{"proof_command": "…", "proof_passed": true, "not_done": ["G9 thunder flashes"],
 "game_dirs": ["…"], "notes": "…"}
```

Then stop. Do not start until you have read `LANE.md`. Do not read or copy any other
Grow a Garden implementation; the reference is the screenshots and this brief.
