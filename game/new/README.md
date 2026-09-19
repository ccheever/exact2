# Small game

`bun proof.mjs` verifies gameplay, Contract text/accessibility and saves on the GPU-less Linux host. `bun proof.mjs web` adds pixels. First run `bun /path/to/exact2/game/prove.mjs .` to fill empty pins after every mode and host agrees; accept later intentional changes with `--repin`.

The simulation defaults to 120 Hz: the measured 60 Hz tick-wait p95 was about 17 ms. Choosing 60 Hz is an explicit latency/CPU trade, not the starter default.

The canvas binds `world(seed=7, paused=false, restart=again)` by the names in
`Options`. Reorder these arguments freely; omitted fields use `Options::default()`.
`world()` takes every default; short positional calls take the remaining defaults.
Raw host JSON key order is not a hash input.

Material synchronization uses the joined `w.query::<(&Beacon, &mut Material)>()` directly: no entity lookup or unwrap. `Material::grid(color, spacing)` supplies the ground; `Environment::default()` supplies height fog and bloom. The beacon uses `Material::default()` and changes its emissive color in the tick; bloom supplies the visible glow.


The template supplies `Cargo.lock` beside `app.json`; the first bake creates ignored
`.shells/` hosts and resolves with `--offline --locked`. Commit that source lock; builds and deployment resolve it locked.
After changing dependencies, update it deliberately with
`bun /path/to/exact2/game/app/shells.mjs . --update-lock`.

Offline resolution requires a populated Cargo cache: materialize `.shells` with the engine README prefetch command, then run `cargo fetch --locked --manifest-path .shells/Cargo.toml`.
