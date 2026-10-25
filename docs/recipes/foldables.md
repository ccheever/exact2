# Recipe: foldables

A partially folded iPhone Duo splits its inner display at the hinge.
`exactViewport()` reports `devicePosture="folded"` with
`horizontalViewportSegments=2` (side by side, a book) or
`verticalViewportSegments=2` (stacked, a laptop).
`env(viewport-segment-<left|top|width|height> <x> <y>)` places a box in segment `x, y` (LLP 1078).

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
- **A pane that does not scroll moves** and keeps its width: centred (full width on a
  narrow screen); the trailing segment on a book fold, where the talk moves an alert.
- **A list and its detail** is a split view (`pattern list-detail`; iOS pushes until
  LLP 1127). **Peer panes** use an iOS 27.1
  [hatch](../contract-for-agents.md#arrange-existing-content-on-iphone-duo).
  Use one tree with CSS elsewhere.
- A route that is itself the pane on the trailing segment keeps its title with
  `navigationBar=(book ? "none" : "auto")`: on a book fold its first-child `header`
  paints in it as written, with its own Back if pushed; otherwise it is UIKit's bar.
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
