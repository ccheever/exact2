# Grow a Garden

A deterministic 3D garden for Godot 4.7. Rules live in `game/sim.gd`. The 3D view and the HUD only display that state.

## Proof

One command, from this directory:

```sh
sh proof/run_proof.sh
```

It runs the headless rule checks (P1–P8, including a fresh process for save and offline catch-up) and then a windowed pass that writes `artifacts/title.png`, `arrival.png`, `planted.png`, `grown.png`, `shop.png`, `backpack.png`, `market.png`, `night.png`, and `rain.png`, plus frame times for about 100 and about 1,000 plants (P9–P10). `GODOT` overrides the engine binary. The default is `/opt/homebrew/bin/godot`.
