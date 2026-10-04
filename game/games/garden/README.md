# Grow a Garden (greybox)

A clone of the Roblox game's loop, built to find where the engine breaks
(diary: [`game/diaries/004-garden.md`](../../diaries/004-garden.md)).

```sh
bun game/dev.mjs garden          # play in the browser
bun game/prove.mjs garden        # verify on the GPU-less Linux host
bun game/games/garden/proof.mjs linux --scale   # the measurements (add --huge for 21,100 plants)
bun game/app/shells.mjs ./game/games/garden --test
```

## Playing

Walk with WASD or the stick. **E** plants the seed in your hand on the tile
under you, or harvests what is ripe there. You start with 20¢ and one carrot.

- **Shop**: fourteen seeds from Common to Divine. Stock rolls every five
  minutes; rarer seeds appear less often. Buy, then hold a seed (`×N`).
- **Backpack**: harvested fruit with weight, mutations and value. Sell one, or
  all. It is a virtualized list, so thousands of fruit stay cheap to show.
- **Garden**: harvest everything ripe, expand the plot (cost grows with size),
  toggle an overview camera; the stress row plants +100/+1,000/+10,000 free
  seeds, jumps an hour away, and toggles per-tick smooth growth.

Plants grow through four stages in real time. Single-harvest crops (carrot,
watermelon, pumpkin, bamboo) are removed when picked; the rest regrow their
fruit. A fruit rolls its size and mutations when it ripens: Gold (1%, ×20),
Rainbow (0.1%, ×50), and the weather's — Rain makes it Wet (×2), Snow Chilled
(×2) or Frozen (×10), a Thunderstorm Wet and Shocked (×100). Value is
`base × (weight/base weight)² × variant × (1 + Σ(environment − 1))`.

Five market orders guide the early garden through carrots, strawberries,
blueberries, tomatoes and corn. Harvest the requested quantity into your
backpack and press **Deliver order**. Each delivery pays the fruit's full
weight/mutation value plus a one-time bonus; unrelated fruit stays in the bag.
The next order and reward are always visible, and completed orders survive saves.
The shop shows first-harvest and regrowth times.

`AI_GATEWAY_API_KEY=… bun proof.mjs web --playtest` (or `macos`) lets Jev try
the first two orders through the HUD and E key. It gets visible text and
enabled controls, with up to 48 decisions; it cannot use stress tools or
write the world. `artifacts/<host>/jev-*` holds the transcript, outcome and
screenshot. This is exploratory play on the agent clock, not deterministic
proof or a claim about visual perception or real-time input latency.

## How it is built

- **Garden time** is world time plus every span the garden spent away
  (`GardenClock`). Everything that will happen — a plant's next stage, a fruit
  ripening, the restock, the weather — is an entry in one saved min-heap
  (`Schedule`), so a tick costs the events due in it, not the plants. Being
  away runs the same loop over the whole span at once: an offline catch-up and
  playing through the same span process the same events in the same order and
  draw the same random numbers (`tests/sim.rs: away_is_the_same_as_playing_through`).
- **Offline growth** comes from the host's wall clock: Contract passes
  `exactTime().epochAtZero` as the live `epoch` argument; a world restored in a
  later session whose epoch is ahead of where its own clock says it is grows by
  the difference. The proof restores the same save an hour later (`--epoch`).
- **Commands** (buy, sell, deliver, harvest, expand, fill, away) are messages:
  Contract's `postMessage("buy carrot", "world")`, read once each, in order,
  from `input.messages()`.
- **The HUD** is three published records — status, shop, backpack — each
  republished only when it changes (status at most once a garden second).
  The backpack is published 200 rows a page: a 164,000-fruit backpack would be
  a 13.6 MB field rebuilt on every harvest.

| File | |
|---|---|
| `logic/src/lib.rs` | arguments, setup, the tick |
| `logic/src/garden.rs` | the clock, the schedule, plants, fruit, weather |
| `logic/src/farm.rs` | tiles, purse, backpack, commands, offline catch-up |
| `logic/src/shop.rs`, `crops.rs`, `hud.rs` | stock, the catalogue and values, publication |
| `logic/tests/sim.rs` | the hostless game tests |
| `logic/tests/scale.rs` | ignored measurements (`--ignored --nocapture`, release) |
| `app.contract` | title, top bar, shop, backpack, tools, prompts, touch controls |
| `proof.mjs` | the real-host proof, `--scale` and Jev's `--playtest` |

![The garden in play](artifacts/web/game.png)
