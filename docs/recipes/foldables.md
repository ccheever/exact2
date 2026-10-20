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

component Note
  resource viewport = exactViewport() as shape Viewport
  derive book = viewport.horizontalViewportSegments == 2
  state draft = ""
  action write(value: string)
    draft = value
  view
    main width="100%" height="100%" position="relative"
      // One pane: on a book fold, the trailing segment
      column position="absolute" top=0 bottom=0 left=(book ? "env(viewport-segment-left 1 0)" : "0px") width=(book ? "env(viewport-segment-width 1 0)" : "100%") box-sizing="border-box" padding=16 gap=12 testId="pane"
        text "Note" role="heading"
        textarea value=draft input=write aria-label="Note" testId="note"
```

- Keep controls off the band between the segments: it is the hinge. Scrolling content
  may cross it.
- **This recipe puts one pane on the trailing segment by default** (segment 1, left to
  right), as [Apple's adaptive-layout talk](https://developer.apple.com/videos/play/tech-talks/111463/)
  moves an alert there, so it stays under the outer display as the device closes. Exact
  moves no pane by itself. The side is yours: segment 0 keeps the pane leading.
- On a book fold UIKit moves neither its bar nor a list (iOS 27.1 simulator). A route
  that is itself the moved pane keeps its title with
  `navigationBar=(book ? "none" : "auto")`: folded, its first-child `header` paints in
  it as written, with its own Back if pushed; flat, it is UIKit's bar.
- Two panes go in reading order: the starting one on the leading segment, the next on
  the trailing one. Segments count from the left, so in a right-to-left language the
  leading segment is 1.
- Propped up like a laptop (two segments down), controls go on the bottom segment, as
  Apple's talk says: cap a bottom sheet's `max-height` at both the room below its header
  and `calc(100% - env(viewport-segment-top 0 1))`, with its containing block from the
  viewport's top. On iOS the keyboard covers that segment and ends the viewport above
  it, leaving the second cap nothing: duo-lab's note sheet lifts that cap then.
- Change the values on one node, as above, so a fold keeps the field and its text. UIKit
  can still end the editing when the device switches panels.
- On the iOS 27.1 simulator, UIKit moves an alert clear of a book fold, and a page or
  form sheet to the left segment (left-to-right text), its right edge about 12 points
  into the band. On a laptop fold a page or form sheet spans it. The tab bar stays
  clear of the fold. Apple's talk says menus and popovers do too.
