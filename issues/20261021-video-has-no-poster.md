# A video shows a blank box until AVPlayer draws: no built-in video with a poster

**Status:** Open
**Systems:** native views, docs (`docs/recipes/native-views.md`), Apple host
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Brooks four-way port's device measurements
**Date:** 2026-10-21

## Symptom

Brooks' Home hero is a looping muted video. On an iPhone 17 Pro Max the exact2 port shows a dark box where the video goes for 50 ms to 1 s after the view exists; the UIKit port shows the video's first frame from its first frame.

## Cause

exact2 has no video element, so the port wrote an `AVQueuePlayer` native view (`brooks-video`, its `modules/apple/Brooks.swift`). AVPlayer's first frame takes 50–1000 ms on a cold launch (measured with a `isReadyForDisplay` observer; the app process is idle meanwhile). The UIKit port covers that wait with a still of the first frame under the player layer. Nothing in exact2 suggests doing so, and the lane also hit two naming traps writing the view (`src` and `playing` are reserved on a native tag).

With a poster layer under the player, the port's hero appears at 0.39 s against UIKit's 0.32 s (`~/bench/brooks-device/launch/results-exact-poster.txt`).

## Fix

Give exact2 a `video` element on the HTML model: `src`, `poster`, `muted`, `loop`, `autoplay`, `playsinline`, `object-fit`, drawn on Apple by AVPlayerLayer with the poster under it until the first frame is ready. Until then, show the poster pattern in `docs/recipes/native-views.md`'s video example, with property names a native tag admits.
