# 2026-11-07 — route scroll owners

## Rough

The route shell named `scroll-${entry.id}` for many pages even when the page
rendered a differently named list, or no single scrollport. Status-bar taps and
tab reselects therefore could not find the intended native scroll owner.

## Lean in

The route shell now uses one Contract function that maps each route to the
scroll or list it actually renders. Missing owners were given stable IDs on the
existing native scrolls: Arrange, agents, terminal capture, attachment
documents, composer settings and choices, scheduled settings, usage account, New Task environment/branch
pickers and the empty archive. Routes backed by a native surface intentionally
return an empty owner.

## Needed

A fresh iPhone and Duo drive is still needed to verify that UIKit's status-bar
and tab-reselect actions reach these owners on device, especially while the Duo
rail is present.

## Checkpoints

- Contract build passed with the existing lower-priority warnings.
- Focused mobile suite: 216 passed, 1 intentional skip, 0 failed.
- iOS no-install app build passed from this checkout.
- The focused native XCTest command reached the existing fixture packaging
  failure (`root-imports.contract` is absent from the generated fixture), so no
  native XCTest result is claimed for this round.
