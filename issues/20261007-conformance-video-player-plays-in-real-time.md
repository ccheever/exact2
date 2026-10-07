# Chrome conformance: the video player plays in real time, and a Linux pointer mismatch

**Status:** Open
**Systems:** host/web-js/conform.mjs, apps/video-player, host/linux
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

Strict JS-target conformance (Chrome, wasm against JS) on main at 931f9fb7d: 16 failures across 81 targets.

- **video-player, 11.** `slots.position`, `derives.shown`, `derives.filled` and `progress-fill.w` differ between the wasm and JS runs: wasm 0.254755 against js 0.000082 at boot, then fractions of a second at each step. The video now plays (autoplay, today's video player commits), so its position is wall-clock time and differs between two runs. The fixture should start paused, or the comparison should leave playback position out for this app.
- **interaction-gallery, `linux resources.media.pointer`: wasm `coarse`, Linux `fine`** (`drag swipe-card-1`, `drag deck-handle`, `tap mode-reorder`). The Linux reference reports a fine pointer where the wasm run under Chrome reports coarse. One of the two hosts reports the wrong pointer for the same launch.

Log: `~/exact2-verify/exact2-async/target/async/931f9fb7de91/conform.log` on the mini.
