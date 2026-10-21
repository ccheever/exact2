# Recipe: foldables

A foldable (the iPhone Duo) splits its inner display at the hinge when partially folded.
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

component Reply
  resource viewport = exactViewport() as shape Viewport
  derive book = viewport.horizontalViewportSegments == 2
  state draft = ""
  action write(value: string)
    draft = value
  view
    main width="100%" height="100%" position="relative"
      // One pane: on a book fold, the trailing segment
      column position="absolute" top=0 bottom=0 left=(book ? "env(viewport-segment-left 1 0)" : "0px") width=(book ? "env(viewport-segment-width 1 0)" : "100%") box-sizing="border-box" padding=16 gap=12 testId="pane"
        text "Reply" role="heading"
        textarea value=draft input=write aria-label="Reply" testId="reply"
```

- Keep controls off the band between the segments, the hinge; scrolling content may
  cross it.
- Scrolling content alone on its screen (an article, a document, a feed, a list) stays
  where it is on either fold, as
  [Apple's adaptive-layout talk](https://developer.apple.com/videos/play/tech-talks/111463/)
  says and UIKit's list does.
- **This recipe puts a pane like the reply on a book fold's trailing segment by
  default** (segment 1, left to right), as the talk moves an alert there, near the outer
  display as the device closes; the other keeps it leading. A route that is itself the
  pane on the trailing segment keeps its title with
  `navigationBar=(book ? "none" : "auto")`: on a book fold its first-child `header`
  paints in it as written, with its own Back if pushed; otherwise it is UIKit's bar.
- On iOS a route moved off a window edge is still padded as if it reached it: its top
  on a laptop fold, its right on a book fold's segment 0.
- Two panes go in reading order, the first on the leading segment: segment 1 in
  right-to-left text, as segments count from the left.
- Folded like a laptop, controls you place yourself go on the bottom segment, as the talk
  says: cap a bottom sheet's `max-height` at both the room below its header and
  `calc(100% - env(viewport-segment-top 0 1))`, with its containing block from the
  viewport's top. On iOS a keyboard over that segment leaves its top below the
  viewport, 0 tall (Chrome moves or drops it): duo-lab's note sheet lifts that cap.
- Place panes by segment edges, ending one under the keyboard at `100%` or
  `bottom: 0`: hosts differ on segment heights.
- Change values on one node so a fold keeps a field's text; UIKit may still end editing
  as the device switches panels.
- UIKit's alerts and sheets on a fold: [Folds on iOS](../contract-for-agents.md#folds-on-ios).
