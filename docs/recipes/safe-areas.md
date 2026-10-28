# Recipe: safe areas

A route, sheet or split column lays its content out in UIKit's safe area: below the bars,
beside a sidebar or the iPhone Duo's side strip, above the home indicator. A scroller or a
plain fill touching that area's edge reaches the screen's edge by itself; its content stays
at the safe area's edge, not at the platform's margins. A plain list, a header's colour, a
sheet's list: write nothing.

```contract
column navigationKey="inbox" position="absolute" inset=0 display="flex" flex-direction="column"
  // Content at the platform's margins (16 on an iPhone, 20 beside a sidebar); the list
  // still spans the screen and UIKit draws its indicator at the display's edge
  scroll flex=1 padding-left="var(--exact-layout-margins-inset-left)" padding-right="var(--exact-layout-margins-inset-right)"
    // A carousel: nothing to write. It spans the screen and scrolls under the sidebar
    list display="flex" overflow-x="scroll" overflow-y="hidden" gap=12
      …
  // A composer: in flow after the list, it follows the keyboard; controls alone make
  // it a bar the list scrolls under and clears; with text or a colour, add the bar value
  row gap=8
    input …
    button …
  // A floating button the list clears, by its own size
  button "New" position="absolute" bottom=16 right=16 -exact-safe-area="bar"
```

- **Content at the platform's margins**: pad the scroller by the values, as above. A carousel
  inside still spans the screen; rows whose colour should span it pad themselves instead.
  `--exact-readable-content-inset-left` and `-right` give a readable width.
- **Two panes side by side**: pad the left one's left and the right one's right by the
  values; the space between them is your `gap`.
- **Keep a box off the edges**: a margin on it, or padding on the route's root. A card with
  a corner radius or a border stays in its box.
- **Lay a box out under the bars or a sidebar** (a map, a photo, a dimming scrim):
  `-exact-safe-area="ignore"` takes every edge; `ignore start` only the leading side,
  `ignore top` only the top. MapKit keeps its controls clear.
- **Keep an image's size but let it seem to continue** under a sidebar: `extend`.
- **A photo at the top of a scrolling page, under the bar**: `navigationBar="transparent"`
  on the route. Inside a scroller, `ignore top` and `extend top` do nothing.
- **A box the content beneath must clear** (a floating button, a card over a map): `bar`.
- `start` and `end` are the leading and trailing sides; right to left mirrors them. The
  values name physical sides: `-left` goes with `padding-left`.
- Padding by `env(safe-area-inset-*)` inside a route clears the safe area twice (warning
  `lower-env-in-owner`).
