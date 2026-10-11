# Recipe: driving iOS

Edit and check on the web host (`test web`, `agent web`: about a second a build), and
drive iOS at milestones, several checks to a native build: one drive in dark mode, one
after a write. `test ios` and `agent ios` build first when the build is missing or older
than its sources, and say so in one line; its log is `.exact/ios-build.log`. The first
build takes minutes, later ones a minute or two, so start the first early, in the
background; a drive started meanwhile waits for it.

```sh
bun exact.mjs ios > .exact/ios.log 2>&1 &      # the first build; $! is its PID
bun exact.mjs test ios
bun exact.mjs agent ios --storage s1 --chrome platform "clock data" "check both"
bun exact.mjs agent ios --storage s1 --chrome platform "prefer prefers-color-scheme dark" \
  "clock data" "screenshot .exact/ios-dark.png window"
bun exact.mjs agent ios --storage s1 --chrome platform "tap add-item" "clock settle" \
  "type add-title Milk" "tap add-save" "clock data" "clock settle" "screenshot .exact/ios-saved.png window"
```

| Flag or op | What it does |
| --- | --- |
| `check`, `check dark`, `check both` | What is wrong with the screen, in text, one line each: text clipped, a control covered (a bar, the keyboard, a sheet) or overlapping, a target under 44 pt, a view past the screen's side, text under its contrast floor, a card that does not show. Run it before a screenshot; keep screenshots for milestones. |
| `--chrome platform` | UIKit's navigation bar, tab bar and sheets, as a person sees them. Without it the authored header and tablist paint instead; a node the bar would not show is marked in `tree`, and a `tap` or `expect` on it fails. |
| `screenshot <file> window` | The whole screen, bars included. Without `window` a sheet is captured alone, at full width, so its type looks twice its size. |
| `clock settle` | Ends a push's, a sheet's or an alert's transition: after opening or closing one, and before tapping inside an alert (UIKit runs an alert button's handler after the alert has gone). A screenshot waits for UIKit's own motion (a segment, a switch) by itself; `screenshot x.png over 600 every 50` films it instead. |
| `clock +1500 real` | Waits real time, for a write on the real clock; never in a test. |
| `"prefer prefers-color-scheme dark"` | Dark mode, as the drive's first op (`simctl ui … appearance` does not reach the drive). |
| `--timing platform` | UIKit's transitions keep their own timing (default: frozen). |
| `--touch platform` | Every tap and drag a real touch, through the XCTest runner, aimed where UIKit draws the target: a bar item, a segment, a list's cell. |
| `--sim <udid\|name>`, `EXACT_SIM` | Which simulator, for `ios` and the drive alike; several booted iPhones are refused, not guessed. |
| `--device`, `--phone <udid\|name>` | A connected iPhone, after `bun exact.mjs ios --device`. |

- Drive by `testId`, never by coordinates. A `testId` on a bar button, the search field,
  a segment or a tab is its `accessibilityIdentifier`.
- `tap` and `type` scroll a target that is out of view, or under a bar or the keyboard,
  into view first, a grouped list's row included: never restructure a screen for the driver.
- Without `--touch platform`, `tap <scroll or list> drag 0 -300` scrolls it as a finger
  would, with no fling; any other drag is refused, naming the flag.
- `tree --ax` prints the accessibility tree as VoiceOver reads it.
- `bun exact.mjs ios --run` opens the app in Simulator.app, to look by hand.
- Before calling it done, drive each write once on the real clock:
  [tests](tests.md#before-you-call-it-done).
