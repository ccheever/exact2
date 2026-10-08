---
name: 20261008-x56-press-lost-on-focus-restyle
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: []
upstream_url: null
reproduced_on: ba541958d (feat(example)/t3-code-usage-pooled-view with b7761f556 merged; main 1f19b2400's framework)
---

# X56: a mouse click whose mouse-down focuses a button that restyles itself on `focus` loses its press (macOS, unconfirmed)

## Summary

On the macOS host, a `button` with `press`, `focus` and `blur` handlers, whose `focus` handler sets a
state that changes the button's own style (a `box-shadow` focus ring), does not run `press` for a
mouse click that starts while the button is not focused. The mouse-down focuses it (the `focus`
action runs and the journal records it), and the click ends there: no `press` is journalled. The
same click on the same button while it already holds the focus presses it, Return/Enter presses it,
and a sibling button without `focus`/`blur` handlers presses on its first click. The expected result
is the web's: one click focuses and presses.

## Why this issue arose

### The T3 Code behavior
`UsageLimitsPooled.tsx` PoolSegment is a Base UI PopoverTrigger: a click opens the account's popover,
and its `focus-visible:ring-2` shows only for keyboard focus.

### What exact2 does today
Driven with the agent on the branch's build (2026-10-08, `scripts/agent.mjs` `open()`, lane ports
16410-16421), journal lines from `logs`:

- `tap usage-seg-0-0-0` on an unfocused segment: `focus view 2692 (focusChanged#7102)`, then nothing;
  the popover stays closed (screenshots `d19`, `d21`, `d24`, `d28`).
- `type usage-seg-0-1-1 key Enter`: `focus view 2872 (focusChanged#7102)`, then
  `press view 2872 (pin#7027)`; the popover opens.
- `tap usage-legend-0-1-1` (a LegendRow button with `press` and `hover`, no `focus`/`blur`):
  `press view 3200 (pin#7027)` on the first click.
- In an earlier drive the same segment, tapped while it already held the focus (given back by
  `focus()` after a confirm), opened its popover on the click.

A one-file app has not been tried (the session budget of this task was spent); the excerpt to try:

```contract
component Root
  state focused = false
  state presses = 0
  action focusChanged(inside: bool)
    focused = inside
  action pressed
    presses = presses + 1
  view
    column
      button press=pressed focus=focusChanged(true) blur=focusChanged(false) testId="b" box-shadow=(focused ? "0 0 0 2px #1b4ed8" : "none")
        text `${presses}`
      button testId="other"
        text "other"
```

`agent.mjs macos "tap other" "tap b" "tree b"`: expected text `1`.

### Where the clone hits it
The pooled segment's own keyboard ring. The clone draws no ring of its own there: the host's focus
ring for custom pressables (X47, main #189) stands in for `focus-visible:ring-2`
(`usage-pooled.contract` PoolSegment).

## Why it must be resolved
Any custom control that draws its focus state from `focus`/`blur` (the clone's bars, rows and chips
did before #189) silently drops the first mouse click. A person sees a ring and no action.
