# Small game

`bun proof.mjs` verifies gameplay, Contract text/accessibility and saves on the GPU-less Linux host. `bun proof.mjs web` adds pixels. First run `bun /path/to/exact2/game/prove.mjs .` to fill empty pins after every mode and host agrees; accept later intentional changes with `--repin`.

Rust hostless tests read simulation time with `sim.world().now()` and save with
`sim.save()`. JavaScript app proofs inspect the running host with
`await session.world('world').snapshot()`.

The simulation defaults to 120 Hz: the measured 60 Hz tick-wait p95 was about 17 ms. Choosing 60 Hz is an explicit latency/CPU trade, not the starter default.

The canvas binds `world(seed=7, paused=paused, restart=again)` by the names in
`Options`. Reorder these arguments freely; omitted fields use `Options::default()`.
`world()` takes every default; short positional calls take the remaining defaults.
Raw host JSON key order is not a hash input.

Every proof ends with its look: each world drawn offscreen (on this machine's GPU,
even from the GPU-less Linux host), its tells, and `LOOK` paths to open once before
you report. A frame that is near black, near white or clipped, particles that cover
nothing or a shadowed sun with no caster fail the look result, printed beside the
simulation's (`simulation: PASS · look: PASS, 0 tells`); a look the game intends is
silenced by name in `app.json` (`"game": {"look": {"silence": ["near-black"]}}`). A
world with no picture or a measurement not taken leaves the look `UNVERIFIED`. The
proof also shows the scene from above (`look(session, {top: 30}, 'plan')`).
Capture the picture below with `bun proof.mjs web --screenshot-only` at `artifacts/web/game.png`.
This reports `UNVERIFIED` because it skips gameplay checks; `bun proof.mjs web` runs the full browser proof.
The grid, sky-colored height fog, sun shadows, beacon pads and bloom are starter defaults.
Colours are CSS, as in `app.contract`: `Material::hex("#e7aa59")`, `Color::hex("#bad6d1")`;
`Material::rgb` takes linear light, where 0.5 is not mid-grey.
Victory uses the beacon count from setup; extending the Rust scene list needs no Contract edit.
Gameplay reloads preserve the running world. To apply edits inside `setup`, pause
and choose **Restart**; setup runs again and play resumes from the beginning.
`Glow(Tween)` on a `Material::glow` mesh is sampled by the renderer at frame time;
the tick only retargets the tween. The scene steps `Follow` after your tick.
`scene::follow(w)` remains optional when you need to choose an earlier ordering.

![Starter proof capture](artifacts/web/game.png)

One `nearest_xz_mut` result supplies both the lighting write and the HUD name;
`w.count::<Beacon>(|b| b.lit)` supplies the total. The proof checks W against
`d = a*m*(m+1)/(2*h*h) + (n-m)*v/h`, where `m=min(n,floor(v*h/a))`.
Character uses constant acceleration up to its speed limit, so this is the sum
of its semi-implicit velocity steps, with a 1 mm float tolerance. Hash pins still
check the exact saved simulation. Space and Enter on the focused Pause button
activate that button without reaching the world.

This game is the files you write: `logic/src/lib.rs`, `app.contract` and
`proof.mjs`, with `logic/tests/` for hostless tests and the tool-written
`pins.json`. Identity derives from the directory name and `Game::ID`; add an
`app.json` only for keys you author (a title, `game.audio`, a bundle id). The
bake generates everything else, ignored, under `.shells/`. Linux proofs use the
incremental `gpu-dev` profile; web, Apple and deploy retain their production
profiles. `EXACT_GAME_PROOF_PROFILE=release bun proof.mjs` checks Linux against
release without changing the game.

Run the Rust tests, from a fresh clone too, with
`bun /path/to/exact2/game/app/shells.mjs . --test`: it generates `.shells/`,
resolves against the SDK's lock offline and locked, and checks the SDK's
determinism lints (exact2's `game/README.md`, "Determinism — the contract").
To depend on another crate, write `logic/Cargo.toml` (`package.workspace =
"../.shells"`, SDK crates as `exact-game.workspace = true`), then capture
this game's own lock with `bun /path/to/exact2/game/app/shells.mjs . --update-lock`
and commit `Cargo.lock`.
