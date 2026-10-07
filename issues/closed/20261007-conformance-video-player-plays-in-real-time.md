# Chrome conformance: the video player plays in real time, and a Linux pointer mismatch

**Status:** Closed
**Resolution:** Conformance freezes media time (mediaClock 'frozen': rate 0 from first load); video-player 11 failures to 0; the Linux pointer mismatch moved to 20261007-web-touch-drag-turns-pointer-coarse
**Systems:** host/web-js/conform.mjs, apps/video-player, host/linux
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

Strict JS-target conformance (Chrome, wasm against JS) on main at 931f9fb7d: 16 failures across 81 targets.

- **video-player, 11.** `slots.position`, `derives.shown`, `derives.filled` and `progress-fill.w` differ between the wasm and JS runs: wasm 0.254755 against js 0.000082 at boot, then fractions of a second at each step. The video now plays (autoplay, today's video player commits), so its position is wall-clock time and differs between two runs. The fixture should start paused, or the comparison should leave playback position out for this app.
- **interaction-gallery, `linux resources.media.pointer`: wasm `coarse`, Linux `fine`** (`drag swipe-card-1`, `drag deck-handle`, `tap mode-reorder`). The Linux reference reports a fine pointer where the wasm run under Chrome reports coarse. One of the two hosts reports the wrong pointer for the same launch.

Log: `~/exact2-verify/exact2-async/target/async/931f9fb7de91/conform.log` on the mini.

## What was done

Conformance's Chrome pages now open with `mediaClock: 'frozen'` (scripts/agent.mjs `open`, host/web-js/conform.mjs): an init script sets a media element's `defaultPlaybackRate` and `playbackRate` to 0 at its first `loadstart`, so a playing video holds its position instead of following the wall clock, while play, pause and seeks still happen as the app drives them and are compared. A first try, Chrome's `--autoplay-policy=user-gesture-required`, was not enough: the drive's first tap is a user gesture, after which the video plays and the positions diverge again. On the mini, `conform.mjs video-player --build --strict`: 11 failures before, 0 after. The Linux pointer mismatch is split out to 20261007-web-touch-drag-turns-pointer-coarse.
