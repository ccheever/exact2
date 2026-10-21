# Game render tests construct Frame without headroom

**Status:** Closed
**Resolution:** Fixed by 38100b2e1; all six render-test Frames provide headroom: 1.0.
**Systems:** game render, exact-gpu
**Severity:** P2
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** LLP 1100 D12b; gpu/src/lib.rs Frame.headroom

`exact_gpu::Frame` gained `headroom` (LLP 1100 D12b). Six game-render literals still construct it without that field, so those test targets do not compile:

- `game/render/tests/look.rs` (`frame`, line 14)
- `game/render/tests/presentation.rs` (`frame`, line 11)
- `game/render/tests/offset.rs` (line 52)
- `game/render/tests/rig.rs` (line 81)
- `game/render/tests/lod_bench.rs` (line 107)
- `game/games/wind-fixture/render/tests/wind.rs` (`frame`, line 9)

`cargo test` of `exact-game-render`'s `look` target stopped with `missing field headroom` at `look.rs:14`. The other literals are the same shape. Neighbouring tests already pass `headroom: 1.0`. The game workspace is outside the root `default-members`, so the five checks stay green.

Set `headroom: 1.0` on each literal (SDR white, matching `Frame`'s own comment). `game/render/tests/hdr.rs` is the test that varies it.
