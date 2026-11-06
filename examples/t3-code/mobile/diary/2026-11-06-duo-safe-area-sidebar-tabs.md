# Duo safe areas and sidebar tabs

## Rough

Split layouts used the raw viewport width. Settings and several other routes used fixed horizontal padding, and the split sidebar had no visible tab rail.

## Lean in

The mobile layout now carries the window's horizontal safe-area insets into split geometry. Settings surfaces and ordinary connection routes add those insets to their authored padding. Sidebar Home chrome provides a retained native tab bar for split navigation.

## Needed

A Duo and iPad drive must confirm that the child route safe area agrees with the window metrics through folding, rotation and inspector transitions.

## Checkpoints

- `bun test layout-mobile.test.ts`
- `bun exact.mjs contract build app.contract --json --errors-only`
- iOS Duo drive: Settings, Home tabs, Thread, Files inspector and rotation
