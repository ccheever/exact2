# 2026-11-05 — tab bar and route viewport

## Rough

The iOS compact Home chrome mounted its UIKit tab controller under the root
overlay. Exact's route content is a scroll viewport, so an empty or loading
Home could carry the native bar with that viewport. The bar also kept the PR
selection after navigating back to Home.

## Lean in

The tab controller now mounts in the route controller's view, above the route
content, with a transparent container and explicit front ordering. Home and
other route scroll owners keep stable IDs while their data is loading or empty.
The compact chrome resets UIKit's selected item to Chats whenever its Home
route is attached again.

## Needed

The TestFlight build must be checked on the internal tester device for the
reported cold-launch flash. The simulator build now shows Home content and a
fixed tab bar together, and tab navigation returns to Home with Chats selected.

## Checkpoints

- Focused scroll contract tests: 5 passed.
- Native iOS simulator build and launch: passed on `t3-standard-pro-current`.
- Tab selection and back-to-Home interaction: passed on the same simulator.
