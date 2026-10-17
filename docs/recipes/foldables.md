# Recipe: foldables

A foldable (the iPhone Duo) splits its inner display at the hinge when partly open.
`exactViewport()` says how, in the web's names: `devicePosture` is `"folded"`, and
`horizontalViewportSegments` (side by side, a book) or `verticalViewportSegments` (one
above the other, a laptop) is 2. `env(viewport-segment-<left|top|width|height> <x> <y>)`
places a box on segment `x, y` (LLP 1078).

```contract
shape Viewport
  width: number
  height: number
  devicePosture: string
  horizontalViewportSegments: number
  verticalViewportSegments: number

component Note
  resource viewport = exactViewport() as shape Viewport
  derive book = viewport.horizontalViewportSegments == 2
  state draft = ""
  action write(value: string)
    draft = value
  view
    main width="100%" height="100%" position="relative"
      // One pane: on a book fold, the trailing segment (1, left to right)
      column position="absolute" top=0 bottom=0 left=(book ? "env(viewport-segment-left 1 0)" : "0px") width=(book ? "env(viewport-segment-width 1 0)" : "100%") box-sizing="border-box" padding=16 gap=12 testId="pane"
        text "Note" role="heading"
        textarea value=draft input=write aria-label="Note" testId="note"
```

- Keep controls off the band between the segments: it is the hinge. Scrolling content
  (an article, a feed, a list) may cross it.
- **This recipe puts one pane on the trailing segment by default** (segment 1, left to
  right), as [Apple's adaptive-layout talk](https://developer.apple.com/videos/play/tech-talks/111463/)
  moves an alert there, so it stays under the outer display as the device closes. Exact
  moves no pane by itself, and UIKit's own sheets and alerts still take the leading side
  on the iOS 27.1 simulator. The side is yours: segment 0 keeps the pane leading.
- Two panes go in reading order: the starting one on the leading segment, the next on
  the trailing one. Segments count from the left, so in a right-to-left language the
  leading segment is 1.
- Propped up like a laptop, Apple's guidance can move a lone element to the top segment.
- Change the values on one node, as above, so a fold keeps the field and its text. UIKit
  itself can still end the editing when the device switches panels.
- The platform's own sheets, alerts, menus and tab bar keep off the hinge by themselves.
