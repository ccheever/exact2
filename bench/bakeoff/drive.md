# Driving an app on the simulator

`bun bench/bakeoff/drive.mjs --udid <UDID> --app <BUNDLE ID> <op> [args] <op> [args] …`

Ops run in order in one call; the first failure stops the call, prints `error: …` and exits 1.
Touches are real finger touches through iOS (XCTest), so the app handles them as it would a person's.
Points are the app's points (the screen is about 402 × 874), origin at the top left.

| Op | What it does |
|---|---|
| `launch` | goes to the Home Screen, ends the app if running, starts it, returns once its screen can be read |
| `relaunch` | the same; use it to check what the app kept across a restart |
| `terminate` | ends the app |
| `home` | presses Home (the app goes to the background) |
| `tree` | prints what is on screen, one element a line (below) |
| `tap 120,300` | taps that point |
| `tap "Save"` | taps the element whose label, id or placeholder is `Save` (exact first, then ignoring case, then part of a label) |
| `type "milk"` | key presses into whatever has focus (tap the field first); `\n` is Return |
| `drag 200,700 200,300 [speed]` | presses, moves to the second point at `speed` points/s (default 1000), lifts |
| `swipe up` | a quick mid-screen swipe the way the finger moves: `up` scrolls down the page |
| `wait 500` | waits that many milliseconds (animations, loading) |
| `screenshot out.png` | saves the screen as a PNG (status bar included) |
| `appearance dark` | switches the simulator to `dark` or `light` |
| `textsize largest` | sets Dynamic Type: `default`, `largest`, or a category such as `extra-extra-extra-large` |

## Reading `tree`

```
Application "Groceries" [0,0 402x874]
  NavigationBar id="Groceries" [0,62 402x106]
    Button "Reset" [351,66 27x36]
  Cell [16,168 370x52]
    TextField value="Bre" placeholder="New item" focused [32,183 338x22]
  Switch "Notify" value="1" [16,272 370x52]
```

Each line: type, label in quotes, then `id=`, `value=`, `placeholder=`, the flags `disabled`, `selected`,
`focused` when they apply, and the frame `[x,y widthxheight]`. Indentation is containment. Wrappers with no
text are left out, an on-screen keyboard is one line, and elements wholly off screen are not listed (swipe to
reach them). A switch's `value` is `1` on, `0` off.

## Habits

- Chain ops in one call: `tap "Add" wait 400 tree`. Each call costs about a second to start.
- After a tap that animates (a push, a sheet), `wait 500` before `tree` or `screenshot`.
- When `tap "label"` finds several matches it lists them; tap one by its point (the middle of its frame).
- An element with no label can only be tapped by its point; that it has none is itself worth noting.
  Content an app draws without exposing it (a canvas) is missing from `tree`: use `screenshot`, then `tap x,y`.
- `type` cannot tell whether the keys reached a field: check with `tree` (the field's `value`) or a screenshot.
- `textsize` and `appearance` stay set on the simulator: put them back (`textsize default`, `appearance light`).
- Typical times: `tap` 0.3 s, `tap "label"` 0.4 s, `type` 0.3 s, `tree` 0.05–0.1 s, `launch` 5 s; the first
  gesture of a call costs up to a second more.
