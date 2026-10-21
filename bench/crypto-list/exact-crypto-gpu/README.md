# Crypto list — exact2, GPU canvas (`gpu`)

The crypto list benchmark (`../SPEC.md`) on exact2 with a GPU `canvas` per row (LLP 1009), built from the checkout
this directory is in. Bundle id `dev.exact.cryptobench.gpu`; a series names it `gpu`. Last measured in the
2026-09-29 series at origin/main dedbe6e8; since then its Contract lost the `writes` clause on `tick`, and its
surface takes the host's command encoder and writes its uniform through `exact_gpu::FrameUniform` (LLP 1009 D7),
as surfaces on current main do. It is unmeasured in that form.

- `app.contract` — the screen. `CoinRow` is the row; `Chart` is the only chart-specific component
  (`canvas surface=spark(c.series, c.up, freeze)`, 114 × 50 at −9 margins: the pulse reaches 9 pt past the
  96 × 32 box, and a canvas cannot paint outside its box). The flash is a second copy of the price faded by
  `transition="opacity 400ms ease"` when the next tick switches it off (`../GAPS.md` §3).
- `data/` — the DataSource: the 5,000 coins (`data/coins.json`, copied by `../prepare.sh`), `config()`, `tick()`;
  each row carries its series as a list. Prices are formatted here.
- `gpu/` — the LLP 1009 module, loaded after the first pixel: `Spark` draws the trimmed polyline, dot and ring in one
  full-canvas fragment pass (distance-based AA; each pixel tests only the 2–3 segments near its x), with the
  pipeline shared by all instances. The draw-in starts at the instance's first frame; the pulse is timed by
  `Frame::now_ms`; `render` returns true forever unless frozen. Its test reads back frames
  (`cargo test --manifest-path bench/crypto-list/exact-crypto-gpu/Cargo.toml -p exact-crypto-gpu-gpu`).
- `apple/` — the static library; `build.rs` bakes the plan and the compatibility id.

## Build and run

From the repository root, after `bench/crypto-list/prepare.sh`:

```sh
EXACT_APP_DIR=$PWD/bench/crypto-list/exact-crypto-gpu EXACT_SIM=<udid> bun host/apple/build.mjs --ios exact-crypto-gpu-apple --run
bench/crypto-list/build-exact.sh gpu    # device: build/CryptoExact2GPU.app, signed with the probe
```

`resign.sh` keeps the GPU module's signature when build.mjs already signed it with the same team (the app checks
the module's digest), so the device build must be signed by the same team as `BENCH_SIGN_IDENTITY`.
