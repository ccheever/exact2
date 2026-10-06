Default deploys republish the web root (2026-09-24, deploy lane): two cold deploys
of unchanged Caltrain left every native stream `current` but changed the
`.exact/install/` pages, which carry their build time, and `app.wasm` and
`gpu_bg.wasm`, which recorded the run's target path until 113abeaa remapped it.
Browsers fetch the new root; native clients are unaffected. The Rust module build
remaps only its target directory: an app outside this repo with a module (none
yet) would also need the capture root its exact2 path dependencies arrive under
remapped, as `wasmRemapFlags` does for web builds.
