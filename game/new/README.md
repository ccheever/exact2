# Small game

`bun proof.mjs` verifies gameplay, Contract text/accessibility and saves on the GPU-less Linux host. `bun proof.mjs web` adds pixels. First run `bun /path/to/exact2/game/prove.mjs .` to fill empty pins after every mode and host agrees; accept later intentional changes with `--repin`.

The simulation defaults to 120 Hz: the measured 60 Hz tick-wait p95 was about 17 ms. Choosing 60 Hz is an explicit latency/CPU trade, not the starter default.

The canvas binds `world(seed: 7, paused: false, restart: again)` by the names in
`Options`. Reorder these arguments freely; omitted fields use `Options::default()`.

Material synchronization uses the joined `w.query::<(&Beacon, &mut Material)>()` directly: no entity lookup or unwrap. `Material::grid(color, spacing)` supplies the ground; `Environment::default()` supplies height fog and bloom; `fog: Some(Fog::new(0.012, 0.1))` explicitly sets exponential density and height falloff. The renderer already applies bounded receiver-plane shadow bias per cascade. `Material::glow(color)` on a sphere gives an emissive beacon and the default bloom supplies its glow.

The first bake creates ignored `.shells/` hosts and captures `Cargo.lock` beside
`app.json`. Commit that source lock; builds and deployment resolve it locked.
After changing dependencies, update it deliberately with
`bun /path/to/exact2/game/app/shells.mjs . --update-lock`.
