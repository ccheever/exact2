# placement-fixture

Simulation tick and save hashes live in [pins.json](pins.json). Run
`bun game/games/placement-fixture/proof.mjs linux --paranoid` and
`bun game/games/placement-fixture/proof.mjs web` from the repository root.
For an intended change, `bun game/prove.mjs placement-fixture --repin` requires
continuous, Save and FreshGame agreement on Linux and web before replacing pins.
Uncited screenshots, receipts and saves stay under ignored `artifacts/`.

`bun game/prove.mjs placement-fixture --hosts linux,web` compares the captured
initial and moving `(x,y,w,h)` tuples directly with a **0.5 px** tolerance.
Each host also checks an independent projection oracle within **0.5 px**. Ordinary
playback uses tick 59 at alpha zero; paranoid reconstruction resets history every
tick and uses the tick-60 endpoint. Both are checked by `logic/tests/sim.rs`.
`--capture40` captures visual evidence;
the previous 0.829/0.963 ms figures did not establish CPU cost and are withdrawn.
