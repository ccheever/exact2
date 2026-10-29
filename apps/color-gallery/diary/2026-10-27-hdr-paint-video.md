surfaces: ios, macos, web
agent: Codex
task: verify HDR paint, native references and video on a physical iPhone

### Rough

- `CALayer.render(in:)` captures the HDR gradient as white and omits the box
  shadow. The same native gradient captures as white, while iPhone Mirroring
  shows both gradients and the shadows. The capture is not the compositor.
- Video diagnostics echoed the requested limit without reading AVKit or the
  inline player layer. `appliedDynamicRange` now reports those objects.
- Hatch diagnostics are shared by word; the session keeps 16 published names.
  Grouping the three limits under each paint kind keeps the 27 probes readable.

### Lean in

- The gallery shows UIKit references beside the same content made with Exact.
  Agent-only probes read the visible layers; an ordinary launch does not sample.
- Three-second clips with a moving bar make playback progress easy to see.
  The existing fixture generator writes SDR H.264, PQ and HLG HEVC Main10,
  and P3 SDR HEVC. Tests check their metadata and decode all 90 frames.

### Needed

- HDR paint: provided. The layer colors, ranges and displayed results agree
  with the native paths tested here; float capture has the limitations above.
- HDR video: provided through AVPlayerLayer and AVKit; actual range reporting
  needed an addition to the host diagnostics.

### Checkpoints

- iPhone 16 Pro, iOS 27.0.1 (24A446), driven through the device agent and
  inspected through iPhone Mirroring. Controlled probes use P3 and headroom 4.
- Background, text, text shadow, ordinary and drawn borders, box shadow,
  gradient, SVG fill and SVG stroke: all three limits reach the painting
  layers as high, constrained-high and standard. Extended colors remain
  present. Float samples retain values above 1 for the seven paths captured
  with extended range; the gradient and box shadow use the native comparison.
- The native and Exact gradients have stops at 1 and 4 and the same range
  settings. Both capture as 1 on this phone; both display a gradient in
  Mirroring. This resolves the earlier capture discrepancy without changing
  the gradient renderer. The native shadow also captures as black while
  displaying the same visible shadow as Exact.
- PQ HEIC under no-limit: 150 matching locations inside the six uniform
  patches give identical RGB values at the device agent's reported precision,
  in extended linear
  sRGB output for Exact and `UIImageView`. This compares patch interiors,
  not resampling at patch boundaries or panel luminance.
- All four video profiles play through both renderers, report a decoded
  frame and advancing time, and have no player error. Only PQ and HLG report
  HDR. Changing standard, constrained and no-limit updates the actual layer
  or controller each time: 24 profile/renderer/limit combinations.
  A Mirroring click selects PQ and starts both players; AVKit exposes its
  native playback controls on a tap.
- The selected iOS suite passes 92 tests, including fixture metadata,
  90-frame decoding and repeated limit changes on both video presentations.
  The earlier broad suite's unrelated keyboard/navigation failures are
  recorded in `2026-10-27-iphone-hdr.md`.
- All six repository checks pass, including the whole-workspace compile.
- macOS launches and reports the actual standard range on both players.
  The web launches with native-only fixtures hidden. The tvOS bundle builds
  and signs with `--no-install`; this does not test tvOS playback.

Reproduce with `bun host/apple/build.mjs --ios --device color-gallery-apple --run`.
The `video-sdr`, `video-pq`, `video-hlg`, `video-p3` and `limit-*` controls select
the cases. Inspect `state.media[].state.appliedDynamicRange` and
`state.hatches.words["hdr-probe"].published`; each paint kind contains its
three limits. `native-no-limit`, `native-constrained` and `native-standard`
name the reference views.
