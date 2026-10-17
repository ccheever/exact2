# Recipe: driving iOS

Edit and check on the web (`verify`: seconds); run `verify ios` once, at the end: the tests with
UIKit's bars, `check` light and dark, and a relaunch ([tests](tests.md#before-you-call-it-done)).
A clean `verify ios` is done. Drives build first when the build is stale (log
`.exact/ios-build.log`); `--prebuild` starts the first early.

An edit to only `.contract` files or what `app.ts` imports reaches iOS in seconds: the drive bakes
the new plan and runs it on the build you have (a bake error fails it, log `.exact/ios-plan.log`).
`app.json`, assets, fonts and new grants rebuild. Before done: `bun exact.mjs ios`, then `verify ios`.

```sh
bun exact.mjs ios --prebuild
bun exact.mjs verify ios
bun exact.mjs agent ios --storage s1 "tap add-item" "clock settle" \
  "type add-title Milk" "tap add-save" "clock data" "clock settle" "screenshot .exact/ios-saved.png window"
```

**Matching a port's screens to the original's screenshots**: `bun
exact.mjs agent ios --touch platform … "clock settle" "screenshot .exact/x.png compare ref/x.png"`
captures as a device does, the keyboard too on a simulator it booted (status bar: `xcrun simctl status_bar <udid> override --time 9:41`) and
says where they differ: each region by node, where its content moved, a box measured in both
(in `x.diff.png`, left to right: reference, screen, differences). Each tap is a real touch: it picks from a pull-down menu,
switches tabs, reaches a native module's controls. A context menu, a row scrolled out of its
menu and an alert's buttons take a drive without the flag.

| Flag or op | What it does |
| --- | --- |
| `check`, `check dark`, `check both` | What is wrong with the screen, a line each: clipped text, a covered control, a small target, low contrast. |
| `--chrome agent` | Paints the authored header and tablist. |
| `--scale 3` | Screenshots at the device's pixels (default 1 a point). |
| `screenshot <file> window` | The whole screen, bars included; a plain one is the window's too while an alert, sheet or menu is up. `screenshot <f> over 600 every 50 window` films it. |
| `tap <id> down` `tap hold <ms>` `tap up` | Press and hold (hold to charge): `pointerdown`, the clock moved `<ms>`, `pointerup`. |
| `clock settle` | Ends a transition; waits up to 1.5 s for the app's own `UIView.animate` (longer: `settled: false`). |
| `--timing platform`, `--touch platform` | UIKit's own transition timing (default: frozen); every tap, hold and drag a real touch. |
| `--sim <udid\|name>` (`EXACT_SIM`) | Which simulator (default: the one the app is on). |

- Drive by `testId` (a bar button's, search field's, segment's, tab's
  `accessibilityIdentifier`), never by coordinates.
- `tap` and `type` scroll their target into view first.
- Without `--touch platform`, `tap <scroll or list> drag 0 -300` scrolls as a finger would, with
  no fling, past the screen too; any other drag is refused, naming the flag.
- A tap on an alert's or action sheet's button returns once UIKit has dismissed it and the press
  has landed, so a `clock data` next sees it.
