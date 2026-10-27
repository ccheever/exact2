surfaces: ios
agent: Codex
task: verify wide color and HDR rendering on a physical iPhone

### Rough

- Image diagnostics read the clipping layer, whose contents are empty, instead
  of its bitmap child. HDR images reported `current: false` and a standard
  layer. Changing `dynamic-range-limit` also addressed that clipping layer.
- The shared-element flight path did not set the bitmap layer's dynamic range.
- iPhone Mirroring and SDR screenshots cannot qualify panel luminance. The
  agent's floating-point samples read rendered content before the display's
  tone mapping; equal samples under different limits do not prove equal light.

### Lean in

- The gallery puts the same HDR file under all three limits, with no app changes.
- The device agent exposes decoded profiles, bit depth and content headroom,
  and reads extended linear sRGB pixels without reducing them to an SDR PNG.

### Needed

- wide color: provided, including P3 canvas storage.
- HDR images: provided; image-layer selection and flights needed host fixes.

### Checkpoints

- Install and launch: smooth on iPhone 16 Pro, iOS 27.0.1 (24A446).
  An ordinary launch reports P3 and high dynamic range.
- Controlled probes: the agent pins P3 and high range (headroom 4); this is
  a test override, not a measured display headroom.
- CSS swatches: sRGB red reads `[1, 0, 0]`; P3 red reads
  `[1.225, -0.0421, -0.0196]` in extended linear sRGB. Rec.2100 linear
  white patches retain 1, 2 and 4.
- Canvas: the two reds match in sRGB storage and differ in P3 storage,
  including `float16`.
- ISO gain-map JPEG: no-limit and constrained retain neutral samples
  0.1813, 0.9996, 1.9959 and 3.3744. Standard decodes an SDR rendition
  with samples 0.0886, 0.3813, 0.5906 and 0.8469.
- After the host fix, all six HDR formats (ISO gain-map JPEG and HEIC,
  PQ PNG, AVIF and HEIC, and HLG HEIC) report the current bitmap under
  no-limit, constrained and standard, with the corresponding layer range.
  Switching the page's limit changes the gain-map JPEG between HDR with
  headroom 4 and SDR with layer headroom 0. All 36 images have no decode failure.
- Changing the agent's display range from high to standard and back changes
  all 12 HDR-capable comparison columns to SDR and back to HDR.
  The 72 sampled image pixels match the pre-fix values within 0.02.
- Background and return: the gallery resumes on the phone, reports visible,
  and retains the three gain-map layer ranges and current bitmaps. This
  checks resumed rendering state, not resumed panel luminance.
- `HDRImageTests.testMountedAndFlyingImagesUseTheBitmapLayersRange` fails
  on the pre-fix host: current bitmap, range, headroom, live limit changes
  and flight range assertions detect the wrong layer or missing setting.
  After the fix, all 90 selected HDR, color, raster, box-layer and flight
  tests pass on the iOS 27.0 simulator, with no skips.
- The broader pre-fix iOS run also has 13 keyboard and navigation failures
  in its hosted suite (195 pass); it does not qualify the whole Apple host
  as green.
- Physical luminance, compositor tone-mapping differences, video, battery
  impact and a comparison against a hand-built native screen remain unverified.
  The captured HDR gradient midpoint reads 1; this capture alone does not
  distinguish a capture limitation from display rendering and is not a pass.
