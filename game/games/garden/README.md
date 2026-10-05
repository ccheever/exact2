# Grow a Garden

A clone of the Roblox game's loop, built to find where the engine breaks
(diary: [`game/diaries/004-garden.md`](../../diaries/004-garden.md)).

```sh
bun game/dev.mjs garden          # play in the browser
bun game/prove.mjs garden        # verify on the GPU-less Linux host
bun game/games/garden/proof.mjs linux --scale   # the measurements (add --huge for 21,100 plants)
bun game/app/shells.mjs ./game/games/garden --test
```

## Playing

The field sits in a meadow beside an orchard, with a straw-hatted gardener,
leafy crops and shaped fruit. Generated models are shared across plants; the
five scenery entities stay constant as the garden expands. `game.assets` in
`app.json` selects the existing model-capable renderer for these models.
The gardener turns and walks with swinging arms and legs. Successful planting
and feeding kick up dirt; watering raises a can and drops blue water; harvesting
sends the picked fruit into the satchel with a short chime. These gestures,
particles and sounds use the saved game clock and preserve crop timing and prices.
`game.audio` selects the optional sound output.

The Garden panel's **Look** row switches the look of the garden you have grown;
play, saves and the world's hash are identical in each. A look is presentation
only (`art` is a live argument): setup places every look's props as bare poses, and
`Game::present` draws the chosen look (`DrawnMesh`, `DrawnLight`, the camera's
`DrawnEnvironment`). Every look but classic draws baked models that `art.mjs`
writes (run `bun game/games/garden/art.mjs` to regenerate `art/`; the bake turns
it into `.model` assets). They are `Game::STREAMED` and only their own look
prefetches them (`Game::prefetch`): the other looks never download them, and
switching to a look fetches what it shows first, then the rest, outside any first
frame. **Golden hour** and **Storybook** (`looks.mjs`) paint every vertex: a
gardener, can and barrel, an orchard and fence, flowers, a meadow of a few
dozen grass tufts placed thousands of times, hills, and a plant and fruit per
crop, in each look's palette (`assets/looks.level.json`). **Art pass** has a model
per crop and growth stage with far levels of detail,
fruit shapes recoloured per mutation, a picket fence with lanterns, a seed
stall with its keeper, a ten-minute day and night, and rain and snow.

Walk with WASD or the stick. **E** plants the seed in your hand on the tile
under you, or harvests what is ripe there. You start with 20¢ and one carrot.
The outline marks that tile: cyan when empty, amber while growing or regrowing,
blue after watering, green when fruit is ready. Crops stand to one side of the tile's centre so you
can see them beside your character. Outside the plots, the prompt gives a
direction, WASD key and approximate distance back to the nearest plot. It
updates as you walk and follows the garden when you expand it.
With a seed in hand on an occupied plot, a separate hint points to the nearest
empty plot. A full garden says so and suggests expansion when it is available.
Growth and harvest prompts stay visible, and the direction disappears once
you reach an empty tile or run out of seeds.

**Q** waters the current plot, cutting its remaining growth or fruit wait by
25%. The can holds three doses. A growing plant takes one dose; when it matures,
its fruit can take another. Each new regrowth can be watered once. Ripe fruit,
empty plots and repeated watering spend nothing. The care panel shows what is
ready and points toward the blue barrel just west of the first plot. Stand near
it and press **R** to refill for free. Both actions also have HUD buttons.
Watering is optional; crops still grow while you explore or are away.

**Compost** a chosen backpack fruit to trade it for one dose of plant food,
instead of its sale value. The pouch holds three doses; a full pouch leaves
the fruit untouched. Stand on a growing plot and press **F**, or its **Feed**
button, to make the upcoming fruit weigh 25% more. Feeding a seedling carries
through to its first fruit; feeding a mature plant affects its current unripe
fruit. It stacks with watering and weather mutations, but repeated feeding of
the same growth spends nothing. New regrowth needs another dose. A purple plot
outline and the care panel mark feeding; harvested fruit is labeled **Fed**.
The saved pouch and crop keep their feeding state while you are away.

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
The market shows the next step and offers **Hold requested seed** when you own
the right seed but hold another. The plot readout names your current tile and
crop; move to an empty tile when a fruiting plant still occupies the old one.
Each new request and restock supplies at least one requested seed at its normal
price. Completed orders survive saves. The shop shows first-harvest and regrowth
times.

`AI_GATEWAY_API_KEY=… bun proof.mjs web --playtest` (or `macos`) lets Jev try
the first two orders through the HUD and E key. It gets visible text and
enabled controls, with up to 48 decisions. Add `--full-market` for all five
orders, walking and the first five crops, capped at 96 decisions. Add
`--start-outside` with `--full-market` to walk beyond the north edge before
Jev takes control, testing the public return guidance with the same policy.
It cannot
use stress tools or write the world. `artifacts/<host>/jev-*` holds the transcript, outcome and
screenshot. This is exploratory play on the agent clock, not deterministic
proof or a claim about visual perception or real-time input latency.

`--playtest --compost` is a separate 64-decision Jev scenario: grow a carrot,
compost it, feed the next crop, and keep its larger harvest. It reads the care
panel and each visible compost button's named trade, using ordinary HUD and
keyboard controls. It does not change the earlier market playtest policies.

## How it is built

- **Numbers are data.** The balance (`assets/garden.level.json`: the seed
  catalogue, mutations and their odds, the weather, market orders, restock
  timing, the farm's rules and the camera) is simulation data: a save records
  its identity, so a balance change moves the pins. Every look's sky, lights,
  soil, meadow, palette, the art pass's day and camera (`assets/looks.level.json`)
  are presentation data, in no save. Both are `Game::LEVELS`, decoded once;
  with `bun game/dev.mjs garden` running, an edit reaches the page in about a
  second with no build (the looks redraw the running garden; a balance edit
  starts a fresh one).
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
  republished only when it changes (status timers once a garden second;
    actions, events and crossing a plot boundary publish immediately).
  The backpack is published 200 rows a page: a 164,000-fruit backpack would be
  a 13.6 MB field rebuilt on every harvest.
- **Watering** shifts the effective start and deadline together, preserving
  smooth-growth progress. It queues an earlier event; the old deadline is ignored
  when it comes due. Only this plant and its bounded fruit slots are visited.
  The can, growth clocks and queue are all saved, including during offline growth.

| File | |
|---|---|
| `logic/src/lib.rs` | arguments, setup, the tick |
| `logic/src/art.rs`, `feedback.rs` | classic models, every look's setup and `present` dispatch, saved gestures, action particles and sounds |
| `logic/src/looks.rs`, `pass.rs` | the golden and storybook looks, and the art pass: their props, sky and drawn models |
| `art.mjs`, `looks.mjs`, `kit.mjs` | every look's baked models, written to `art/`; the glTF writer and mesh shapes they share |
| `logic/src/garden.rs` | the clock, the schedule, plants, fruit, weather |
| `logic/src/farm.rs` | tiles, purse, backpack, commands, offline catch-up |
| `logic/src/shop.rs`, `crops.rs`, `hud.rs` | stock, the balance's types and values, publication |
| `assets/garden.level.json`, `assets/looks.level.json` | the balance; every look's lighting, palette and camera |
| `logic/tests/sim.rs` | the hostless game tests |
| `logic/tests/scale.rs` | ignored measurements (`--ignored --nocapture`, release) |
| `app.contract` | title, top bar, shop, backpack, tools, prompts, touch controls |
| `proof.mjs` | the real-host proof, `--scale` and Jev's `--playtest` |

![The garden in play](artifacts/art-final-view-web/grown.png)
