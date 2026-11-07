# 2026-11-06 — native tab-bar policy

surfaces: ios
agent: Codex
task: keep the native workspace tab bar pinned like Expo

## Rough

The documented iOS test command found six booted simulators and stopped without selecting one. Passing `--sim "iPhone 17"` ran the focused suite successfully. The app-local `feedback` verb is not present in this checkout, so no feedback note could be recorded through that command.

## Lean in

The Expo tab host's explicit UIKit policy maps directly to Exact's owned `UITabBarController`, and the focused XCTest suite covers both the policy and the retained tab stacks.

## Needed

- tab bar: provided by UIKit

## Checkpoints

- iOS test build: smooth after selecting a simulator; all `NavigationTabsIOSTests` cases passed.
