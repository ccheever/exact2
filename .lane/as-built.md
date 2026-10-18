# Lane 1132.A2

## D2 — 2026-10-18

The phase table is in [measurement.md](measurement.md). Loaded cold builds
were 1,388.7 s web, 537.0 s native `gpu-dev`, and 902.1 s native release.
The cache-eligible registry dependency chain is 28.3% of native `gpu-dev`,
above the RFC's quarter gate. Web's whole registry chain is 17.8%; release's
eligible chain is 6.9%. Only native `gpu-dev` uses the new cache.

`~/.cache/exact/game-crates` holds two coherent registry generations,
separated by channel, profile, triple, rustc and flags. It excludes path/git
crates and their transitive dependants. A fresh profile takes a generation;
Cargo validates the fingerprints. The integration trial copied 392
fingerprints and rebuilt none of them. Registry compiler invocations fell
from 379 to 34. No engine path artifact is shared. Cache files are outside
proof inputs. Ordinary SDK setup preserves I13; `setup --game` explicitly
opts into compiling the optional engine to warm its registry dependencies.

The game README describes the cache and distinguishes initial shipping
`web` builds from the `gpu-dev` producer used after Rust edits. It names the
wasm-bindgen PATH prerequisite and the native build-only command. Generic
`exact.mjs linux` builds the `host-dev` host only; the measurement instead
uses `EXACT_GAME_PROOF_PROFILE=gpu-dev bun proof.mjs linux --build-only` to
build both the module and host. The Games pitfalls section names this
command distinction.

The public `setup --game` run exited 0. The actual dev watcher selected
`gpu-dev`: its first wasm build took 212.326 s and a warm one-line restore
4.800 s under load 106–136. The source was restored, and a web drive moved
the player and rendered the game; the screenshot was inspected. Handoff to
LLP 1127.001 D1: this watcher is already using the intended profile; initial
`web` host and native producer startup work must be measured separately.

Deviations: the shared machine stayed heavily loaded, so the RFC's three
runs per leg under load 5 are not verified. The requested loaded table is
recorded without extrapolating to quiet performance. No observed load fell
below 10 to trigger a quieter repeat. Native Linux here is its supported
headless host compiled for the Mac's native triple. The release cache was
tested experimentally but is not wired into the product because it did not
earn the quarter gate. The requested contributor-pitfalls file is absent.

Required root and game checks, Bun script tests, all fourteen shell fixtures,
and the Beacons baseline passed. The fresh Beacons attempt exercised the
existing generated-directory refusal; its unchanged stable rerun passed.

## As built

D2 measured the cold web, native `gpu-dev` and native release legs before
adding a cache. Native `gpu-dev` alone earned registry reuse: its eligible
registry dependency chain was 151.7 of 537.0 s (28.3%). Its fresh-target
reuse trial took 417.8 s under a different heavy load, copied 392 registry
fingerprints without rejection, and compiled 34 registry units instead of
379. This is acceptance evidence, not a quiet-machine speedup claim.
Release and web retain their builds. The shipping cache is warmed by the
explicit `exact setup --game` option, never changes authored inputs, and
holds no path crates or their dependants. A measured Rust edit used
`gpu-dev` (warm 4.800 s under load 107–109); LLP 1127.001 D1 owns the dev
loop's remaining performance work. The quiet three-run oracle remains
unverified; all load averages, phases and controls are in measurement.md.
