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

## Verification status

**Browser proof blocked before page load on this machine.** Three launch attempts
failed; the last two explicitly report Chrome exiting with SIGABRT and no stderr.
One used Bun and the other Node with GPU disabled. The underlying abort cause was
not established. `browser-failure.json` records the observations. No screenshot
exists; rendering, DOM/input behavior, and browser save/restore remain unverified.
The full proof script is implemented but has never reached its browser assertions.

`sim-proof-result.json` records 22 passing simulation assertions, including a
fresh Bun process restoring `sim-save.json` and reproducing the continuation
byte for byte. This is a substitute, not a passing browser proof. The predicted
W position after 1.5 s is `(0, 0, -5.616435329305953)` metres. JS syntax checks pass.
No frame pacing, cross-host determinism, or pixel identity was measured.

198 physical lines of game source (155 JavaScript + 43 HTML/CSS); 237 lines of
proof/serving code, reported separately. Diary: `game/diaries/001-beacons-three.md`.

## Live feel probe

The verification notes above describe the original game proof. A separate headed,
live-clock diagnostic is now available from the repo root:
`bun game/bench/feel.mjs three`. `?feel=1` loads the adjacent `feel.js` observer;
the runner arms its buffers, drives real CDP input, and reads them once at the end.
It samples the drawn capsule and camera without editing `game.js` or `simulation.js`.
There is no built-in interpolation switch for this game's fixed-step accumulator.
See [the benchmark's Feel section](../../../bench/README.md#feel--beacons-live-clock)
for all runs, raw traces, limitations, and the event-delivery-to-drawn-state definition.
