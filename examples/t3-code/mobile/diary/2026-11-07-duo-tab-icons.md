# 2026-11-07 — Duo tab icons and rail geometry

## Rough

The TestFlight screenshot showed the Duo tab bar with labels and Settings' gear,
but no glyphs for Chats, PRs or Usage. The empty Home action also sat too close
to the vertical rail when the rail's reserved-region report arrived after the
first geometry packet.

## Lean in

The root tab contract now uses SF Symbols present in the deployed iOS symbol
set: `bubble.left`, `arrow.triangle.pull` and `chart.bar`. Content geometry also
rechecks after the window-level tab bar is installed, allowing UIKit's delayed
reserved-region report to reach the route before the empty state settles.

## Needed

Install a fresh iPhone and Duo build and verify all four glyphs plus the empty
Home/Add Environment spacing. The device-panel launch service was unavailable
during this checkpoint, so the signed compile and contract tests are the local
evidence until that service returns.

## Checkpoints

- Mobile app and content-geometry tests.
- Signed iOS simulator compile from this checkout.
