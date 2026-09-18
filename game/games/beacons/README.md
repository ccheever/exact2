# Beacons

A small third-person game on exact2's game add-on. Move with WASD or arrows, jump
with Space, and press E within 1.5 m of each floating beacon. Light all three and
Play again to restart with the same seeded scenery. Pause freezes world time.

From the repository root:

```sh
sh game/games/beacons/run.sh proof
sh game/games/beacons/run.sh test
sh game/games/beacons/run.sh dev
```

The launcher sets the required CommandLineTools developer directory, development
update trust, and game-local build/output paths. The browser proof runs headless.
Host/GPU adapters are the engine's existing synthesized shells; no game-specific
renderer or host code is needed.

`artifacts/proof.txt` contains assertions; `artifacts/replies.json` contains the
operation replies. `artifacts/beacons-playing.png` is the playing screenshot.
`checkpoint.world`, `run-0.world`, `run-1.world`, and `restored.world` demonstrate
fresh-process restore and byte-identical continuation. Process cleanup is recorded
in `artifacts/process-cleanup.json`.

Movement uses 12 m/s² acceleration, 20 m/s² braking, and a 4 m/s top speed. The
stated 90-tick W position is `[0, 0.9, -5.3666644]`. Gravity is 9.81 m/s²;
beacon glow uses a finite 0.5-second smoothstep tween. Scenery has no collisions.
The glow is material emission, without bloom. No textures or audio are used.
