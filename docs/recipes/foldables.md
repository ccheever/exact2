# Recipe: foldables

On native Duo hosts, `exactViewport()` reports `devicePosture="folded"` and
`horizontalViewportSegments=2` (book) or `verticalViewportSegments=2` (laptop).
Web pages need browser support for these signals; a feature flag alone does not
establish availability. `env(viewport-segment-<left|top|width|height> <x> <y>)`
places a box in segment `x, y`.

```contract
shape Viewport
  horizontalViewportSegments: number

component Reply
  resource viewport = exactViewport() as shape Viewport
  derive book = viewport.horizontalViewportSegments == 2
  state draft = ""
  action write(value: string)
    draft = value
  view
    main width="100%" height="100%" position="relative"
      column position="absolute" top=0 bottom=0 left=(book ? "env(viewport-segment-left 1 0)" : "max(0px, calc((100% - 480px) / 2))") width=(book ? "env(viewport-segment-width 1 0)" : "min(100%, 480px)") box-sizing="border-box" padding=16 gap=12 testId="pane"
        text "Reply" role="heading"
        textarea value=draft input=write aria-label="Reply" testId="reply"
```

On a fold ([HIG](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo#Dynamic-layouts),
[Apple's talk](https://developer.apple.com/videos/play/tech-talks/111463/)):

- Scrolling content can span the hinge; keep controls off it.
- A pane that does not scroll keeps its width: centred (full width on narrow
  screens), or on the trailing segment of a book fold.
- **A list and its detail** is a split view (`pattern list-detail`; iOS pushes until
  LLP 1127). **Peer panes** use [native containers](../contract-for-agents.md#arranged-panes)
  on iOS 27.1 and responsive CSS elsewhere (Duo Lab's Panes tab).
- A route on the trailing segment keeps its title with
  `navigationBar=(book ? "none" : "auto")`: its first-child `header` paints in
  that pane, with its own Back if pushed; otherwise UIKit makes the bar.
- Put the first pane on the leading segment: segment 1 in RTL; indices count from the left.
- Segment coordinates are relative to the viewport. Keep the pane container
  at that origin and chrome inside a pane. End a bottom pane at `bottom: 0` to fit above the page's bars.
  Book panes in a column flex container use absolute positioning, automatic
  left/right insets and `align-self: flex-start` / `flex-end` for reading order.
  Duo Lab uses the smaller segment width to clear unequal hinges. Inset controls
  from safe edges too.
- In a laptop fold, put authored controls on the bottom segment. Cap a bottom
  sheet's `max-height` at the room below its header and
  `calc(100% - env(viewport-segment-top 0 1))`, from the viewport origin.
  An iOS keyboard can leave that segment below the viewport and zero tall;
  Duo Lab's note sheet then lifts the cap.
- End panes under the keyboard at `100%` or `bottom: 0`; hosts report different segment heights.
- Keep the same pane instances to retain field text; UIKit may end editing on a panel switch.
- UIKit's alerts and sheets on a fold: [Folds on iOS](../contract-for-agents.md#folds-on-ios).
