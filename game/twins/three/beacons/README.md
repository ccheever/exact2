# Beacons — three.js r186

From the repository root:

```sh
bun game/twins/three/beacons/run.mjs --serve
# Open http://127.0.0.1:8086/beacons/index.html
bun game/twins/three/beacons/run.mjs            # full Chrome proof
bun game/twins/three/beacons/sim-proof.mjs      # simulation-only substitute
```

WASD or arrows move on world axes; Space jumps; E lights a beacon within
1.5 metres while grounded. Buttons provide Play, Pause/Resume and Play again.
Change `?seed=42` to change the six crates. Interactive play runs a 120 Hz
accumulator with a 250 ms frame catch-up cap; proof mode advances explicit integer
ticks through that same step function. Background stalls therefore slow game time
rather than fast-forwarding the game. No physics engine, textures or sound.

`beacons.save()` returns a JSON string; `beacons.restore(json)` restores it,
including velocities, jump state, held keys, pending actions, camera, beacon easing,
seed/PRNG and fractional accumulator. These are callable in the browser console.
`?proof` enables the manual test clock. The full proof sends actual CDP keyboard
and mouse events and uses fresh Chrome processes/profiles for replay and restore.

Historical proof results and browser limitations are in [the diary](../../../diaries/001-beacons-three.md).

## Live feel probe

A headed,
live-clock diagnostic is now available from the repo root:
`bun game/bench/feel.mjs three`. `?feel=1` loads the adjacent `feel.js` observer;
the runner arms its buffers, drives real CDP input, and reads them once at the end.
It samples the drawn capsule and camera without editing `game.js` or `simulation.js`.
There is no built-in interpolation switch for this game's fixed-step accumulator.
See [the benchmark's Feel section](../../../bench/README.md#feel--beacons-live-clock)
for all runs, raw traces, limitations, and the event-delivery-to-drawn-state definition.
