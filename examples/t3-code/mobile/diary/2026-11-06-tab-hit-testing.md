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

The workspace wrapper now passes pointer input through to the selected tab panel
and its direct route columns. The selected panel remains interactive and hidden
tab panels remain inert, so the four app tab stacks stay mounted without a
full-screen wrapper stealing touches. Settings remains a root tab with the
native header's Close control; pushed Settings routes keep the tab stack and
native Back behavior.

## Needed

The local device-agent session was unavailable during the parent verification
attempt. A reviewer drive on the signed iOS build reached Settings → Appearance
and returned with platform Back while retaining the Settings root; a fresh
parent device capture is still useful when the agent service is available.

## Checkpoints

- Contract build: passed.
- `app.test.ts`: 31 tests, 131 assertions passed.
- iOS no-install build: passed.
- Reviewer native drive: Settings root and the persistent tab bar were present;
  Appearance pushed and platform Back returned to the retained Settings root.
