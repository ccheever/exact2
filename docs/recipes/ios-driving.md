# Recipe: driving iOS

**Check a screen against a reference** (the original's screenshot) with compare,
not by eye: `bun exact.mjs agent ios --touch platform … "clock settle" "screenshot
.exact/x.png compare ref/x.png"` captures as a device does, the keyboard too on a simulator it
booted (status bar: `xcrun simctl status_bar <udid> override --time 9:41`), and says where they
differ: each region by node, where its content moved, a box measured in both (`x.diff.png`:
reference, screen, differences). Each tap is a real touch, reaching a pull-down menu's
rows, tabs and a native module's controls. A context menu, a row scrolled out of its
menu and an alert's buttons take a drive without the flag.

Edit and check on the web (`verify`); run `verify ios` once, at the end: the tests with
UIKit's bars, `check` light and dark, and a relaunch ([tests](tests.md#before-you-call-it-done)).
Drives build first when stale (log `.exact/ios-build.log`); `--prebuild` starts the first
early. With an `app.ts` data module, an edit to `.contract` files, `app.ts` or what it imports
reaches iOS in seconds as a baked plan (log `.exact/ios-plan.log`); `app.json`,
assets, fonts, `app.ts`'s grants, `appId` or canvas surfaces and, with a Rust data crate, a file
the build reads rebuild. Before done: `bun exact.mjs ios`, then `verify ios`.

```sh
bun exact.mjs ios --prebuild
bun exact.mjs verify ios
bun exact.mjs agent ios --storage s1 "tap add-item" "clock settle" \
  "type add-title Milk" "tap add-save" "clock data" "clock settle" "screenshot .exact/ios-saved.png window"
```

| Flag or op | What it does |
| --- | --- |
| `check`, `check dark`, `check both` | What is wrong with the screen: clipped text, a covered control, a small target, low contrast. |
| `--chrome agent` | Paints the authored header and tablist. |
| `--scale 3` | Screenshots at the device's pixels (default 1 a point). |
| `screenshot <file> window` | The whole screen, bars included; a plain one is the window's too while an alert, sheet or menu is up. `screenshot <f> over 600 every 50 window` films it. |
| `tap <id> down` `screenshot <png>` `tap up` | `<id>` shown pressed, then pressed once; `tap hold <ms>` moves the clock; a real touch under `--touch platform`. |
| `clock settle` | Ends a transition; waits up to 1.5 s for the app's `UIView.animate`, 4 s for a glide (longer: `settled: false`). |
| `--timing platform`, `--touch platform` | UIKit's own transition timing (default: frozen); every tap and drag a real touch. |
| `--sim <udid\|name>` (`EXACT_SIM`) | Which simulator (default: the app's). |

- Drive by `testId` (a bar button's, field's, segment's, tab's
  `accessibilityIdentifier`), never by coordinates.
- `tap` and `type` scroll their target into view first.
- A wheel or `tap <scroller> drag 0 -300` moves content that far, bars as a finger would; a
  real drag glides unless held 100 ms ([more](../contract-for-agents.md#drags-on-ios)).
- A tap on an alert's or action sheet's button returns once UIKit has dismissed it and the press
  lands, so a `clock data` next sees it.
