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

Capture the picture below with `bun proof.mjs web --screenshot-only` at `artifacts/web/game.png`.
This reports `UNVERIFIED` because it skips gameplay checks; `bun proof.mjs web` runs the full browser proof.
The grid, sky-colored height fog, sun shadows, beacon pads and bloom are starter defaults.
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

`app.json` contains authored identity and game keys. The bake resolves host defaults
into ignored `.shells/app.json` and leaves the authored file byte-for-byte intact.
Linux proofs use the incremental `gpu-dev` profile; web, Apple and deploy retain
their production profiles. `EXACT_GAME_PROOF_PROFILE=release bun proof.mjs` checks
Linux against release without changing the game.


`Cargo.lock` beside `app.json` is captured package state; the first bake creates ignored
`.shells/` hosts and resolves with `--offline --locked`. Commit that source lock; builds and deployment resolve it locked.
`logic/Cargo.toml` is author-owned after its initial scaffold and is never normalized
by a bake. Add dependencies and adjust paths there directly, including after moving
the game directory.
After changing dependencies, update it deliberately with
`bun /path/to/exact2/game/app/shells.mjs . --update-lock`.

If the Cargo cache is empty, run `bun /path/to/exact2/game/app/shells.mjs .`,
then `cargo fetch --locked --manifest-path .shells/Cargo.toml`.
