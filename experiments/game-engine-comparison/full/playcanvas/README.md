# Lanterns — Last Light (PlayCanvas)

A complete PlayCanvas 3D implementation of the shared Lanterns comparison brief. Ammo.js owns the live rigid-body world: the capsule player collides with the island and obstacles, pushes the dynamic crate, and must climb the crate to reach the ledge lantern. The supplied Fox.glb is loaded as a PlayCanvas container asset and plays its Survey, Walk, and Run clips.

## Build and run

Requires Bun 1.3.14 or compatible.

```sh
bun install --frozen-lockfile
bun run build
bun run start
```

Open <http://localhost:4173>. Production output is in `dist/`. For deterministic inspection, open `http://localhost:4173/?agent=1` and use `window.lanterns.command(...)`.

## Controls

- WASD or arrow keys: move (world-relative)
- Space: jump
- E: light the nearest lantern
- Escape or the HUD button: pause/resume
- On-screen controls are available on touch devices

The pause screen saves and restores the full run through browser local storage. Sound is generated at runtime with Web Audio, so no extra art or audio assets are used.

## Timing

Setup and dependency install are recorded separately from implementation in `evidence/timing.md`.

`evidence/acceptance/acceptance.json` is the independent 10/10 browser acceptance result. The same directory contains title, moving, crate, won, and lost screenshots. `evidence/results.json` is the implementation lane's movement/jump/reload self-check.

## Notes

The camera is a fixed world-relative chase camera; pointer lock is not needed. Browser autoplay policy means sound begins only after a user gesture. Physics is stepped at 60 Hz and rendering runs independently. Agent mode renders on demand so large fixed-tick batches do not spend time painting unused intermediate frames.

The delivered countdown includes a one-line rollover correction made after
trial inputs were frozen. Final UI and gameplay evidence is retained under
`../evidence/playcanvas-ui-final/` and `../evidence/playcanvas-delivery-final/`.
