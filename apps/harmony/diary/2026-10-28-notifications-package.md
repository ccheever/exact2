surfaces: web, ios, macos
agent: Claude Code, Claude Opus 5.5
task: post mention notifications through @exact/notifications, and ask for permission when Notify Me of Mentions turns on

## Rough

A catch-up is a wake, and a wake's post while the permission is `prompt` is refused `person-required`: no prompt can
be raised without a person. The switch therefore asks (`requestNotificationPermission()`) in the press that turns it
on, so the first catch-up finds the answer given. Before, the built-in `showNotification` asked at its first post,
from the wake, and on iOS nothing asked at all (QUEUE, 2026-10-26).

## Lean in

The post itself moved with only a `use` line: the package keeps `showNotification`'s names and `navigate=`.

## Needed

- notifications: provided, by the package; the click opening the room is unchanged

## Checkpoints

- editing app.contract: smooth
- driving the app: `packages/background/test/harmony.mjs` grants the permission before the switch; not run in this change
