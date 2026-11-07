# 2026-11-06 — tab route hit testing

surfaces: ios
agent: Codex
task: keep Settings child routes reachable beneath the persistent app tabs

## Rough

The compact root kept every app tab mounted, but its `workspace-panel` wrapper
still participated in hit testing. On iPhone, a Settings row could therefore
report that the parent covered its middle even though the route itself had an
interactive hit target.

## Lean in

The workspace and tab-panel wrappers pass pointer input through. The selected
route stays mounted for state retention, but its wrapper also passes input
through to the direct header and body row, which explicitly restore interaction
for their controls. Older routes and inactive tab panels remain hidden and
inert, so they cannot cover the active route. Settings remains a root tab with
the native header's Close control; pushed Settings routes keep the tab stack and
native Back behavior.

## Needed

The first signed drive exposed full-screen retained route wrappers covering
Settings, PRs and Usage rows. The corrected Contract gates visibility and hit
testing to the active route and passes input through the route wrapper; a fresh
signed-device drive is required to verify the fix after rebuilding.

## Checkpoints

- Contract build: passed.
- `app.test.ts`: 31 tests, 131 assertions passed.
- iOS no-install build: passed.
