# Recipe: driving iOS

Edit and check on the web (`verify`: seconds); run `verify ios` once, at the end: the tests with
UIKit's bars, `check` light and dark on the first screen and where each test ends, and a
relaunch ([tests](tests.md#before-you-call-it-done)). A clean `verify ios` is done. `verify ios`,
`test ios` and `agent ios` build first when the build is missing or older than its sources (log
`.exact/ios-build.log`); start the first, minutes long, early with `--prebuild`, and keep editing:
it builds the Swift as it was, and the next drive builds the edit.

An edit to only `.contract` files or what `app.ts` imports reaches iOS in seconds: the drive bakes
the new plan and runs it on the build you have (a bake error fails it, log `.exact/ios-plan.log`).
`app.json`, assets, fonts and new grants rebuild. Before calling the app done, run `bun exact.mjs
ios` once (a real build), then `verify ios`.

```sh
bun exact.mjs ios --prebuild                   # the first build, apart from this shell
bun exact.mjs verify ios                       # the tests, check light and dark, a relaunch
bun exact.mjs agent ios --storage s1 "tap add-item" "clock settle" \
  "type add-title Milk" "tap add-save" "clock data" "clock settle" "screenshot .exact/ios-saved.png window"
```

**Matching a port's screens to the original's screenshots** is one tool, with no AXe or `simctl
io`: `bun exact.mjs agent ios --touch platform --scale 3 … "clock settle" "screenshot .exact/x.png
window"`. Each tap is a real touch: it opens UIKit's menus and picks a row (`tap <row's testId>`),
switches tabs, and reaches a native module's own controls, which a plain `tap` refuses.

| Flag or op | What it does |
| --- | --- |
| `check`, `check dark`, `check both` | What is wrong with the screen, one line each: clipped text, a covered or overlapping control, a target under 44 pt, a view past the screen's side, low contrast. |
| `--chrome agent` | Opt-in: paints the authored header and tablist in place of UIKit's bars. |
| `--scale 3` | Screenshots at the device's pixels; `agent` takes 1 pixel a point (402×874). |
| `screenshot <file> window` | The whole screen, bars included; a plain one is the window's too while an alert, sheet or menu is up. |
| `clock settle` | Ends a push's, sheet's or alert's transition and the app's own `UIView.animate`; a tap waits for an overlay the app laid on the window (a splash) to go. |
| `--timing platform`, `--touch platform` | UIKit's own transition timing (default: frozen); every tap and drag a real touch, where UIKit draws the target. |
| `--sim <udid\|name>` (`EXACT_SIM`), `--device`, `--phone <udid\|name>` | Which simulator (default: the booted one the app is on), or a connected iPhone. |

- Drive by `testId`, never by coordinates; on a bar button, the search field, a segment or a tab
  it is the `accessibilityIdentifier`.
- `tap` and `type` scroll a target out of view, or under a bar or the keyboard, into view first:
  never restructure a screen for the driver.
- Without `--touch platform`, `tap <scroll or list> drag 0 -300` scrolls as a finger would, with
  no fling; any other drag is refused, naming the flag. `tree --ax` prints the accessibility tree.
