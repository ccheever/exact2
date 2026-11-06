# Home projection cache

## Rough

Home redraws arrived with a fresh frame clock during scrolling, but the complete
shelf projection was rebuilt for every redraw. That repeated row sorting, search
projection and menu preparation even when the client and fleet had not changed.

## Lean in

`mobileHomeView` now retains one complete view per client after its first
canonical revision. The cache is keyed by the client and fleet revisions,
thread epoch, sampled second, route/surface inputs, catalog state and the saved
catalog identity. Revision zero remains uncached so setup fixtures can continue
to populate objects in place. A clock-second change or any canonical revision
invalidates the view.

The root snapshot also reconciles the local backend's catalog pass before
publishing metadata. A let-go-aware preparation handle keeps the background
hydration pass from calling `native.watch` after its answer has retired.

## Needed

Native scroll traces with a populated Home remain needed to quantify the frame
budget on an installed device. This change does not alter the Home action owner
or the transcript projection cache.

## Checkpoints

- 2026-11-06: focused Home, scroll and cache tests pass (23 tests, 5,928 assertions).
- 2026-11-06: full mobile suite passes (4,312 tests, 2 skipped); rebuilt iOS runtime shows the offline Home state with the tab bar as a sibling outside the scroll area.
- 2026-11-06: post-fix iOS rebuild survives a light/dark appearance change and Add environment navigation without the retired-answer banner; captures are `/tmp/t3-mobile-parity-home-light.png` and `/tmp/t3-mobile-parity-add-environment-dark.png`.
