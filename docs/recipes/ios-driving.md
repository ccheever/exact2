# Recipe: driving iOS

Edit and check on the web (`verify`: seconds); run `verify ios` once, at the end: the tests with
UIKit's bars, `check` light and dark on the first screen and where each test ends, and a
relaunch ([tests](tests.md#before-you-call-it-done)). A clean `verify ios` is done. `verify ios`,
`test ios` and `agent ios` build first when the build is missing or older than its sources (log
`.exact/ios-build.log`). The first build takes minutes, so start it early with `--prebuild`: it
returns at once and a drive started meanwhile waits for it.

An edit to only `.contract` files or what `app.ts` imports reaches iOS in seconds, with no native
rebuild: the drive bakes the new plan and runs it on the build you have (a bake error fails it,
log `.exact/ios-plan.log`). `app.json`, assets, fonts and new grants rebuild; a file `app.ts`
never imports, such as `server/`, rebuilds nothing. Before calling the app done, run `bun
exact.mjs ios` once (a real build), then `verify ios`.

```sh
bun exact.mjs ios --prebuild                   # the first build, apart from this shell
bun exact.mjs verify ios                       # the tests, check light and dark, a relaunch
bun exact.mjs agent ios --storage s1 --chrome platform "tap add-item" "clock settle" \
  "type add-title Milk" "tap add-save" "clock data" "clock settle" "screenshot .exact/ios-saved.png window"
```

| Flag or op | What it does |
| --- | --- |
| `check`, `check dark`, `check both` | What is wrong with the screen, one line each: clipped text, a control covered (a bar, the keyboard, a sheet) or overlapping, a target under 44 pt, a view past the screen's side, low contrast, a card that does not show. |
| `--chrome platform` | UIKit's navigation bar, tab bar and sheets, as a person sees them. Without it the authored header and tablist paint instead; a `tap` or `expect` on a node the bar would not show fails. |
| `screenshot <file> window` | The whole screen, bars included. Without `window` a sheet is captured alone, at full width. |
| `clock settle` | Ends a push's, sheet's or alert's transition: after opening or closing one, and before tapping inside an alert. |
| `--timing platform`, `--touch platform` | UIKit's own transition timing (default: frozen); every tap and drag a real touch through XCTest, aimed where UIKit draws the target. |
| `--sim <udid\|name>` (`EXACT_SIM`), `--device`, `--phone <udid\|name>` | Which simulator (several booted iPhones are refused), or a connected iPhone after `bun exact.mjs ios --device`. |

- Drive by `testId`, never by coordinates; on a bar button, the search field, a segment or a tab
  it is the `accessibilityIdentifier`.
- `tap` and `type` scroll a target that is out of view, or under a bar or the keyboard, into view
  first: never restructure a screen for the driver.
- Without `--touch platform`, `tap <scroll or list> drag 0 -300` scrolls as a finger would, with
  no fling; any other drag is refused, naming the flag. `tree --ax` prints the accessibility tree
  as VoiceOver reads it; `ios --run` opens Simulator.app.
