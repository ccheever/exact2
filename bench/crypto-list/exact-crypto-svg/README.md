# Crypto list — exact2, inline SVG (`svgi`)

The crypto list benchmark (`../SPEC.md`) on exact2, built from the checkout this directory is in. Bundle id
`dev.exact.cryptobench.svgi`; a series names it `svgi`. This is the exact2 app of the 2026-09-30 series
(`crypto-svgi@9fdf7e564`), with its Contract brought to current main (the `writes` clause on `tick` removed).

- `app.contract` is the whole screen. `CoinRow` is the row; `Chart` is an `svg` (96 × 32, `viewBox="0 0 96 32"`,
  `overflow="visible"`) with a `polyline` whose `points` the view maps from the row's 48 prices (`chartPoints`,
  LLP 1017.003), `pathLength=1` and `stroke-dasharray="1"` with `animation="draw 600ms ease-out both"` over
  `stroke-dashoffset` 1 → 0; a ring disc (`breathe`: `r` 3 → 9, opacity 0.5 → 0, 1,200 ms ease-out, 600 ms delay,
  infinite) and the dot (`shown 600ms step-end both`: absent during the draw-in). Under `BENCH_FREEZE` the ring is
  static at r 6, opacity 0.25.
- The price flash is a keyed CSS animation (`fade 400ms ease both` on opacity) on a coloured copy of the price: it
  starts when the copy is inserted, so the colour shows at once and fades over 400 ms.
- `data/` is the DataSource: the 5,000 coins (`data/coins.json`, copied by `../prepare.sh`, not committed,
  `include_bytes!`'d), `config()` (`BENCH_LIVE` / `BENCH_SCENARIO=rest` / `BENCH_FREEZE`), `tick()` (the SPEC's live
  ticks; unchanged rows keep their `Rc` records). Prices are formatted here; the series stays numbers.
- `apple/` is the static library, whose `build.rs` bakes the plan (the coins are the resource's compiled boot value)
  and the compatibility id. `apple/tests/points.rs` checks every coin's `chartPoints` against the Rust `{:.2}`
  string the first SVG app formatted (the same coordinates, so the same pixels).
- `Cargo.toml` is its own workspace with exact2's `[patch.crates-io]` lines. Its `Cargo.lock` is not committed:
  `../prepare.sh` copies the root's, which build.mjs then resolves for this workspace.

On iOS the draw-in, pulse and flash run in Core Animation (LLP 1055 D7): no display link at rest for them.

## Build and run

From the repository root, after `bench/crypto-list/prepare.sh`:

```sh
# simulator (cargo release + swift -c release)
EXACT_APP_DIR=$PWD/bench/crypto-list/exact-crypto-svg EXACT_SIM=<udid> bun host/apple/build.mjs --ios exact-crypto-svg-apple --run
EXACT_APP_DIR=$PWD/bench/crypto-list/exact-crypto-svg bun scripts/agent.mjs ios "clock +2000" "screenshot out.png"
# live ticks: simctl forwards SIMCTL_CHILD_* to the app's environment
SIMCTL_CHILD_BENCH_LIVE=1 xcrun simctl launch <udid> dev.exact.cryptobench.svgi
# device: an unsigned archive, unpacked and signed with the probe into build/CryptoExact2SVGI.app
bench/crypto-list/build-exact.sh svg
# the coordinates test
cargo test --manifest-path bench/crypto-list/exact-crypto-svg/Cargo.toml -p exact-crypto-svg-apple --test points
```
