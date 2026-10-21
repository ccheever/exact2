# Diary

2026-10-06T05:21:19Z — Scaffolded `garden/` with `exact.mjs new --game --assets`. The game SDK crates were not all fetched and Xcode.app is not selected (CLT only); native mac launch is not the proof host.

2026-10-06T05:36:33Z — Implementing the game. Choices, where the brief was silent or the screenshots disagree with a sentence of it:

- Show the weather label even when it is Clear (the screenshots do).
- Plot numbers are 1-based: tile (0, 0) is "Plot 1, 1". The plot pill also lists "W north · D east · S south · A west".
- Summary is always "N plants", including 1. Ripe denominator is fruit slots already set, so a seedling is "0/0".
- Tile outline priority from the screenshots: green ripe, purple fed and not yet rolled, blue if this phase was watered, amber while growing, cyan when empty.
- Holding pill is "No seeds in hand" or "Holding Carrot ×1". The shop row of the held seed shows a dark "×N" and a green highlight.
- Day clock is the position inside the 10-minute cycle, so t=150s is "Day 2:30" and still daylight. Restock counts down from 5:00, next boundary `((ms/300000)+1)*300000`.
- First-fruit line is grow_s+fruit_s ("20s to harvest · one harvest", "70s to first fruit · regrows every 30s").
- Money: under 1000 is `N¢`; otherwise one decimal only when it is not an integer (`3K¢`, `1.2K¢`, `3.4M¢`).
- Value is computed in f64 with `round` (half away from zero) and `ratio*ratio`. f32 is not exact near the big crops.
- One world RNG. Setup order after reseed: shop restock, then the first Clear wait (180–420s). Same-timestamp events: weather, then restock, then plants in tile order. Snow is two rolls (Chilled 0.4, else Frozen 0.1). Rainbow consumes the variant roll and does not also roll Gold.
- Watering scales only future stage times: `now+(old-now)*3/4`. A carrot planted at 0 and watered at 4000 ms matures at 16000 ms. The can spends nothing on ripe, empty, already-watered, or an empty can. R within 2.5 m refills; tile (0, 0) is 2.0 m from the barrel.
- Fed is cleared when the cohort rolls, so a feed before the roll applies once to every slot and a repeat feed spends nothing.
- Game clock is integer milliseconds at 60 Hz: `acc += 1000; add = acc/60; acc %= 60`. Any 216000 ticks add exactly 3,600,000 ms. `advance_by` is the only time stepper, for one tick and for an hour. 60 Hz rather than the starter's 120 Hz so the played hour is 216k ticks.
- `now()` is the session clock, which starts at 0. It is not the epoch. Away is the host gap on the first tick after a restore (`seen_host_ms` saved, `booted` not saved). A gap above 500 ms calls `advance_by` and that tick does not also add its dt. A negative gap rebases `seen` and does not rewind. Pause posts the same away message on resume. The engine's first clock sample after restore has zero world delta, so a seek does not also simulate the hour tick by tick.
- Plants live in the Garden resource. Visual entities are named and reconciled when dirty, so a played hour and a skipped hour can differ in entity ids and emitter age. P8 compares rules and the RNG stream, not those.
- Barrel origin is its centre, so it sits at y=0.5. Character `bounds_xz` is one inclusive range for both axes; the garden is clamped by hand to x ∈ [−20, 2n+20], z ∈ [−(2n+20), 20].
- Grid material lines up on tile centres, so tile lines are thin boxes on the boundaries. Soil sits just above the meadow.
- Mutation look: rainbow hue, else gold metal, frozen pale cyan, wet/chilled darker blue, shocked emissive, stacked where they can. Thunder flashes are a presentation light flicker on top of rain.
- Plants stand at tile centre +(0.55, 0, −0.35) so the gardener does not hide them. Fruit hangs on the mature plant.
- Camera follow offset (0, 11, 14), lag 0.18, from the south. Overview pulls back toward the garden centre.
- Seed is 7, fixed on the canvas. Start selected seed is Carrot.

2026-10-06T06:25:30Z — First Rust build. `Resource` already implements `Data`, so deriving both failed. `Sky` and the row structs needed `Default` because `Data` requires it. Signed `div_ceil` is still unstable, so page count is integer division. Hostless `Sim::new` leaves setup pending until assets load; tests use `Sim::baked`. Unit tests (value, water, hour) and the two sim tests passed. Proof is `garden/proof.mjs`. A fresh boot tick does add its ~16 ms; only a restore gap over 500 ms skips dt.

2026-10-06T06:32:49Z — Linux proof (`bun proof.mjs`) passed P1–P6, then died on P7 at `tap('play')` of the restored process: `unregistered component FruitLook`. A fresh process runs setup before it loads the save, and fruit meshes (and the fence lanterns) are inserted only on a later sync, so they were missing from the schema. `Game::register` now registers `FruitLook`, `Limbs`, `Lantern`, and the garden resources. A hostless test saves a ripe carrot and restores it onto a new `Sim::baked`; that passed. Cost was one full host compile (~60s) plus this diagnosis. Re-running the Linux proof.

2026-10-06T06:41:38Z — Second Linux proof got through P7 (ticks matched, hashes did not) and then `run(1 hour)` died with `TimerFireLimit { limit: 4096 }`. The HUD stamp was `every(50)`, so one clock span fired more timers than the runner allows, and it also wrote a different `seen_host_ms` into each process, which the world hash includes. Removed the repeating stamp: Play sets the host once. The proof now seeks the restored session to that saved host before Play, and splits long seeks into 60 s steps. The next Linux run passed P1–P9 with 0 failures in 6.9 s and came back `UNVERIFIED` because `pins.json` was empty. Recorded tick 1 `0x724c1aaeb801918d` and save `mid`. P9: 2.9 ms for `run(100)` at 101 plants, 3.3 ms at 1001.

2026-10-06T06:45:17Z — Linux proof with those pins: `PROOF PASS` in 6.6 s. Web proof: `PROOF PASS` in 116.8 s, pins matched, and the nine screenshots are in `artifacts/` and `garden/artifacts/`. Looked at all nine and at `reference/arrival.png`. The given art is a different model set from that reference; the HUD, plot, farmer and panels sit in the same places. Arrival shows Restock 4:59 because the shot is after the boot tick (~16 ms), so the 5:00 countdown has already stepped. The rain shot is the Rain sky, not a thunder frame; thunder still raises the sun when `sin(ms * 0.017) > 0.92`. P9 on web: 4.3 ms at 101 plants, 16.0 ms at 1001. Game code is 3185 lines (Rust, Contract, proof, README), excluding art and generated `.shells`. About four Linux proof runs and one web run to a pass, after the earlier Rust compile failures. Nothing in the brief is left undone.
