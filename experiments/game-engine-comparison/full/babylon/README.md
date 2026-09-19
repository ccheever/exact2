# Lanterns — Last Light · Babylon.js

A complete browser implementation of the common Lanterns comparison contract. Babylon.js renders the island, imported fox, shadows, glow, and sampled animation. Babylon's Havok v2 integration owns all solid collision, gravity, jumping, the player body, and the mass-3 pushable crate.

## Run

The repository pins Bun 1.3.12. The fleet browser proof used Bun 1.3.14, and the same frozen lock plus production build were also verified locally with Bun 1.3.12.

```sh
bun install --frozen-lockfile
bun run build
bun run start
```

`start` binds only to `127.0.0.1`; with no `PORT` environment variable it asks the OS for an unused port and prints a JSON line containing the PID, port, and URL. The checked production build is emitted to `dist/` by Rolldown. It includes the Havok WASM module and exact supplied Fox.glb/license bytes.

## Controls

- WASD or arrow keys: world-relative movement
- Space: jump
- E: light the nearest unlit lantern in range
- Escape or Pause: pause/resume
- On-screen controls support touch. Sound, save, resume, restart, win, and lose controls are visible in the UI.

The crate must be pushed beside the east ledge. Jump onto it, then onto the ledge to reach lantern 12. Keyboard state is cleared when the page loses focus.

## Agent adapter

Open `/?agent=1` to disable wall-clock simulation. `window.lanterns.command(request)` implements the common `ready`, `start`, `reset`, `input`, `step`, `state`, `pause`, `save`, and `load` requests. Each `step` tick advances the live Havok world exactly once at 60 Hz; player/crate transforms and velocities are read back from those bodies. Fox Survey/Walk/Run animation groups are advanced from the same tick clock.

For the fleet proof, start the server, copy its printed URL, and run:

```sh
BASE_URL=http://127.0.0.1:<port>/ bun run test
BASE_URL=http://127.0.0.1:<port>/ bun run test:route
```

The first test drives ordinary adapter input through movement, collection, jumping, pause, dynamic crate pushing, save/reload/load, and timeout. The route test navigates the entire island without teleporting, lights eleven ground lanterns, pushes and climbs the crate, jumps to the ledge, lights lantern 12, and verifies the win. They write JSON evidence and screenshots under `evidence/`.

The independently authored round-two acceptance is preserved under `evidence/independent-round2/`. It passed all 10 cases with no browser errors: imported asset/title, real keyboard input and animation, jump/landing, pause, wall collision, crate/ledge collision, process reload, complete victory route, lose/restart, and tick batching. Its title, moving, crate, win, and lose screenshots were visually inspected.

## Recorded timings and scope

- Browser dependencies already cached from the microfixture comparison: excluded from this lane's setup.
- New pinned dependency setup: 1.48 s for Babylon/Havok/Rolldown/Playwright, then 0.55 s for the matching Babylon glTF loader.
- Implementation wall time was 1,203 s from lane creation through the final all-twelve route proof. Exact setup, build, and verification measurements are in `evidence/timings.json` and keep dependency setup separate.

The camera is fixed in a trailing three-quarter view and movement remains world-relative. Audio uses small Web Audio oscillator cues so the comparison introduces no extra downloaded art or sound assets.
