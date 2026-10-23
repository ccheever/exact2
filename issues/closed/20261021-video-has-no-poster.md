# A video shows a blank box until AVPlayer draws its first frame

**Status:** Closed
**Resolution:** Fixed 2026-10-21: see the ticket's fix section.
**Systems:** native views, docs (`docs/recipes/native-views.md`), Apple host
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Brooks four-way port's device measurements
**Date:** 2026-10-21

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed (2026-10-21)

## Symptom

Brooks' Home hero is a looping muted video. On an iPhone 17 Pro Max the exact2 port shows a dark box where the video goes for 50 ms to 1 s after the view exists; the UIKit port shows the video's first frame from its first frame.

## Cause

Two things. The port wrote its own `AVQueuePlayer` native view (`brooks-video`, its `modules/apple/Brooks.swift`), because `docs/start-here.md` sent "video" to the native-views recipe: exact2's `video` element (LLP 1042), which takes `poster`, `autoplay`, `muted`, `loop`, `playsinline` and `object-fit`, was not named there. And that element's poster would not have covered the wait: `VideoArm` hid it when playback began (`timeControlStatus` playing, or a non-zero rate when the poster landed), not when a frame was ready, so an autoplaying video still showed a blank box for the 50 ms to 1 s AVPlayer takes to draw its first frame on a cold launch (measured with an `isReadyForDisplay` observer). The UIKit port covers that wait with a still of the first frame under the player layer.

With a poster layer under the player, the port's hero appears at 0.39 s against UIKit's 0.32 s (`~/bench/brooks-device/launch/results-exact-poster.txt`).

## Fix

`VideoArm` places the poster by `object-fit`, as the frames are placed (it was always fitted, so under `cover` it jumped when the video replaced it), and keeps it until playback or a seek has begun *and* the presentation (`AVPlayerLayer`, `AVPlayerViewController` or `AVPlayerView`) is `readyForDisplay`, as HTML shows the poster frame until video data is available; a remote poster loads through the shared HTTP cache and is decoded off the main thread. The media snapshot reports `poster` (`none`, `shown`, `hidden`) and `frameReady`. `docs/start-here.md`, the guide's media section and the native-views recipe name the `video` element, with a `poster`, for a video.

Filmed on an iPhone 16 Pro Max (`~/bench/imagebench`, a 420 pt `cover` hero with its first frame as `poster`): before, the hero was blank until 1.1–1.3 s after launch; now the poster is in the app's first frame (0.25 s), as in a UIKit `AVPlayerLayer` with a poster image under it.
