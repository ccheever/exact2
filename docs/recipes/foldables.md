# Recipe: foldables

A foldable, such as an iPhone Duo, splits its inner display at the hinge when partially folded.
`exactViewport()` uses web names: `devicePosture` is `"folded"`, and
`horizontalViewportSegments` (side by side, a book) or `verticalViewportSegments` (one
above the other, a laptop) is 2. `env(viewport-segment-<left|top|width|height> <x> <y>)`
places a box on segment `x, y` (LLP 1078). All Duos run iOS 27.1+, so Duo-only native code
can use APIs such as `UIHinge` behind `#available(iOS 27.1, *)` with no fallback.

```contract
shape Viewport
  width: number
  height: number
  devicePosture: string
  horizontalViewportSegments: number
  verticalViewportSegments: number

component Reply
  resource viewport = exactViewport() as shape Viewport
  derive book = viewport.horizontalViewportSegments == 2
  state draft = ""
  action write(value: string)
    draft = value
  view
    main width="100%" height="100%" position="relative"
      // A book fold's trailing segment, else centred
      column position="absolute" top=0 bottom=0 left=(book ? "env(viewport-segment-left 1 0)" : "max(0px, calc((100% - 480px) / 2))") width=(book ? "env(viewport-segment-width 1 0)" : "min(100%, 480px)") box-sizing="border-box" padding=16 gap=12 testId="pane"
        text "Reply" role="heading"
        textarea value=draft input=write aria-label="Reply" testId="reply"
```

On a fold ([HIG](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo#Dynamic-layouts),
[Apple's talk](https://developer.apple.com/videos/play/tech-talks/111463/)):

- **Scrolling content stays**: a list or document spans the band (the hinge). Keep
  other controls off it.
- **Non-scrolling panes move**, keeping their width: centred (full width if narrow),
  or on a book fold's trailing segment, like the talk's alert.
- **List and detail**: a split view (`pattern list-detail`; iOS pushes until LLP 1127).
  **Peer panes**: place them by segment (duo-lab's Fold tab).
- A route on the trailing segment keeps its title with
  `navigationBar=(book ? "none" : "auto")`: on a book fold its first-child `header`
  paints in it as written, with its own Back if pushed; otherwise it is UIKit's bar.
- Place panes in reading order, leading segment first: segment 1 for
  right-to-left text, since segments count from the left.
- For a laptop fold, place authored controls on the bottom segment, as the talk says.
  Cap a bottom sheet's `max-height` at both the room below its header and
  `calc(100% - env(viewport-segment-top 0 1))`, with its containing block from the
  viewport's top. On iOS a keyboard over that segment leaves its top below the
  viewport, 0 tall (Chrome moves or drops it): duo-lab's note sheet lifts that cap.
- Place panes by segment edges, ending one under the keyboard at `100%` or
  `bottom: 0`: hosts differ on segment heights.
- Change one node's values to keep field text across folds; UIKit may end editing
  when switching panels.
- UIKit's alerts and sheets on a fold: [Folds on iOS](../contract-for-agents.md#folds-on-ios).
