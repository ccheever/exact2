# Duo rail reservation

task: keep responsive thread controls clear of a wide right-side occlusion

### Rough

The responsive content geometry reserved the Duo rail only when an occlusion's
left edge was already past the bounds midpoint. A wide rail or hinge crossing
that midpoint could therefore leave the thread composer and Send control inside
the reserved region.

### Lean in

The trailing reservation now admits an occlusion whose clipped rectangle reaches
the content's trailing edge, even when its left edge is before the midpoint.
The existing right-half rule remains for floating right-side occlusions. The
reservation still uses the first reported matching region, so no global safe
area or keyboard ownership changes.

### Needed

A native Duo drive with a wide or multi-region geometry packet is still needed
to qualify the actual Send hit target while the keyboard is visible. This source
unit makes no frame-rate or native reachability claim.

### Checkpoints

- `content-geometry.test.ts`: 8 tests pass, 1 native test skipped, 86 assertions.
- Added midpoint-crossing and multiple-occlusion reservation regressions.
