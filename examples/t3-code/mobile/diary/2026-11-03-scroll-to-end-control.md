# Scroll-to-end control

## Rough

The pinned mobile feed keeps a live-follow latch and exposes a separate circular
control when a reader scrolls away from the end. The clone had a permanent
`scrollFollowEnd` list attribute and no matching control.

## Lean in

The root now derives the latch from the feed's `ScrollEvent`, keyed by the
selected environment and thread. The scroll-to-end action targets the last row
with the existing keyed `scrollIntoView` command, and the working control keeps
its capsule and arrow as separate glass surfaces.

## Needed

The Contract path is covered by focused action and source tests. Native raster
comparison of the moving glass pair remains part of the broader iOS parity work.

## Checkpoints

- Contract build passes.
- Focused mobile tests pass.
- Full mobile suite is rerun after the route fixture gains the new prop.
