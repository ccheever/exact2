# Home rail reservation

## Rough

Home and thread routes must leave the same trailing space for a wide Duo rail. The shared route geometry already treated an occlusion that crosses the window midpoint as a reservation; Home only looked at the occlusion's left edge.

## Lean in

The Home predicate now accepts either a right-half occlusion or one whose clipped rectangle reaches the content's trailing edge. The reservation remains the distance from the occlusion's left edge to the content edge, matching thread and route geometry.

## Needed

Native keyboard-visible Duo reachability still needs a device drive. This source change prevents Home rows from being covered by a wide trailing rail but does not claim a frame-rate improvement.

## Checkpoints

- Added a Contract regression for a 176-point occlusion beginning at x=300 in a 466-point content box.
- Focused mobile tests and the normal iOS simulator build are the acceptance checks for this slice.
