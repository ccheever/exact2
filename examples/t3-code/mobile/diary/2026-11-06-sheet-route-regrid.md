# 2026-11-06 — partial-sheet route regrid

## Rough

On iOS, Add Environment opened at Expo's 55% sheet detent but its authored
header and form kept the root viewport origin. The sheet therefore showed a
grabber and empty body while the accessibility tree still contained the form.

## Lean in

The modal controller regrids after UIKit attaches and settles a page-sheet
detent. When route siblings move into the presentation mount, their authored
frame is preserved relative to the route instead of being regridded from the
old root origin. This keeps the header, fields and submit action inside the
visible sheet without changing the compact layout.

## Needed

A device-agent drive on a physical Duo should confirm the visible form at both
55% and 70% rests. The simulator test runner built the iOS tests but its
`xcodebuild test-without-building` process stopped producing results after
repeated iOS 27.2 `invalidDigitCount` diagnostics.

## Checkpoints

- Focused mobile navigation, geometry and scroll tests.
- Contract build.
- iOS test target compile from this checkout.
- TestFlight archive and submission from the fixed commit.
