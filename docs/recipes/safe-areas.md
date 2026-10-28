# Recipe: safe areas

A route, sheet or split column lays its content out in UIKit's safe area: below the bars,
beside a sidebar or the iPhone Duo's side strip, above the home indicator. A scroller or a
plain fill that touches that area's edge reaches the screen's edge by itself; its content
stays inside. A list, a header's colour, a sheet's list: write nothing.

```contract
column navigationKey="inbox" position="absolute" inset=0 display="flex" flex-direction="column"
  // Content at the platform's margins (16 on an iPhone, 20 beside a sidebar); the list
  // still spans the screen, and UIKit draws its indicator at the display's edge
  scroll flex=1 padding-left="var(--exact-layout-margins-inset-left)" padding-right="var(--exact-layout-margins-inset-right)"
    // A carousel: nothing to write. It spans the screen, scrolls under the sidebar and
    // rests its first card at the margin
    list display="flex" overflow-x="scroll" overflow-y="hidden" gap=12
      …
  // A bar the list scrolls under and clears: controls in flow after the list
  row gap=8 padding=8
    input …
    button …
```

- **Inset a list's content** with the scroller's own padding: the scroller, and a carousel
  inside it, still span the screen. Rows whose colour should span it too pad themselves
  instead. `var(--exact-readable-content-inset-left)` and `-right` give a readable width.
- **A bar the list scrolls under and clears** (a composer, a filter row): put it in flow
  before or after the scroller, holding only controls, with no background colour.
- **Keep a box off the edges**: a margin on it, or padding on the route's root. A card with
  a corner radius or a border stays in its box; a plain fill that touches the edge reaches it.
- **Lay a box out to the screen's edges** (a map, a full-bleed photo):
  `-exact-safe-area="ignore"` takes every edge. Name edges to take only those: `ignore start`
  is the leading side alone, `ignore top` the top alone. MapKit keeps its controls clear.
- **Continue artwork under a sidebar or a bar**: `-exact-safe-area="extend"` on the image.
  iOS mirrors its edges there; the image keeps its size.
- **A floating button** (`position="absolute" bottom=16 right=16`) sits 16 from the safe
  edges over the list, and follows the keyboard and a minimizing bar. The list does not
  clear it: add its height to the list's `padding-bottom`.
- **The scroll indicator** is UIKit's: it follows the safe area and the bars.
- `start` and `end` are the leading and trailing sides; right to left mirrors them. The
  values name physical sides: `-left` goes with `padding-left`.
- `env(safe-area-inset-*)` inside a route clears the safe area a second time and stops the
  list reaching (warning `lower-env-in-owner`).
