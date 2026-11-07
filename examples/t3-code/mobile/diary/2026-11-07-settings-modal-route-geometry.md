# Settings sheet and route geometry

## Rough

The compact Settings root was still a tab route with a close control, while
tab panels reserved a fixed 56 points below every root. That combination cut
scroll content above the tab bar and did not match the documented Expo sheet
presentation.

## Lean in

The root state now marks compact Settings as the existing `settings` form
sheet and modal presentation. Split layouts retain a full-width detail route.
Tab panels stay full height so UIKit owns the tab bar inset. Secondary routes
consume the measured Duo rail reservation at the shared route shell.

## Needed

The device bridge must recover before the sheet detent, tab-bar overlap, and
Duo rail can be confirmed with platform-chrome screenshots. Source and focused
Contract tests are the current evidence.

## Checkpoints

- 2026-11-07: 155 focused mobile tests passed, one native-geometry test skipped.
- 2026-11-07: Contract build passed with existing lower-priority warnings.
