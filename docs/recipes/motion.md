# Recipe: motion

Motion is CSS's: `transition` for a value that changes, `keyframes` and `animation` for an
entrance or a loop, `-exact-exit-animation` for a node leaving, `-exact-layout-transition` for
a box that moves or resizes.

```contract
keyframes rise
  from opacity=0 translate="0px 8px"
keyframes leave
  to opacity=0 scale=0.96 animation-timing-function="ease-in"
keyframes breathe
  50% opacity=0.4

style Row
  -exact-layout-transition="320ms cubic-bezier(.32,.72,0,1)"
  -exact-exit-animation="leave 200ms ease-in both"

shape Media
  prefersReducedMotion: bool

component App
  resource media = exactViewport() as shape Media
  derive still = media.prefersReducedMotion
  state items = ["Kick", "Snare", "Hat"]
  state open = false
  action toggle
    open = not open
  action remove(name: string)
    items = items.filter(i => i != name)
  view
    column padding=16 gap=8
      button press=toggle testId="toggle"
        text "Details"
      text "Shown" opacity=(open ? 1 : 0) translate=(open ? "0px 0px" : "0px -8px")
        transition="opacity 200ms ease, translate -exact-spring(240, 26, 1)"
      for name, i in items key=name
        row class=Row gap=8 animation=(still ? "none" : `rise 300ms ${i * 60}ms both`)
          text name flex=1
          button press=remove(name) testId=`remove-${name}`
            text "Remove"
      text "Saving…" animation=(still ? "none" : "breathe 1.6s ease-in-out infinite")
```

- **What animates:** translate, scale, rotate, opacity, colours, `box-shadow`, SVG geometry,
  a numeric `height`, `font-weight`, `font-stretch`, `font-variation-settings`. Other
  layout properties (`width`, `top`) are refused, naming `-exact-layout-transition`. A
  native `button` animates only opacity and transforms.
- **`-exact-spring(stiffness, damping, mass)`** is a transition's timing, with no duration;
  an `animation` refuses it.
- **Keyframes** take literals or the app's own `fn` on constants.
- **Exit:** `-exact-exit-animation` names keyframes and must end (`infinite` is refused). It
  plays when an `if` arm turns false or a `for` row leaves: the node leaves layout at once,
  inert, and the rest move by their layout transition. Not inside a screen being popped,
  nor on Linux.
- **Layout transition** moves a box and resizes its surface (background, border, shadow)
  with no property name. Children jump, unscaled. Compute its value with
  ``still ? "none" : `${ms}ms ease` ``. `sharedElement=` flies from the old box on iOS,
  macOS and the web, not Linux.
- **One after another:** delays (`"a 200ms both, b 300ms 200ms both"`, the stagger above by
  `i`). A state change when it ends is a gated task's `after(ms, action)`. A `sequence`
  is sound and haptics, not motion.
- **Reduced motion:** transitions and keyframes run unless the app gates them on
  `exactViewport()`, as above (a shared-element flight on iOS and macOS lands at once); drive it
  with `prefer prefers-reduced-motion reduce`.
- **Check it:** `clock settle` ends the motion; `screenshot .exact/m.png over 600 every 50`
  films it.
