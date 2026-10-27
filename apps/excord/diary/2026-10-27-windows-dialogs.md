surfaces: windows, linux
agent: Claude Code (Claude Opus 5.5)
task: Excord's switcher, profile, member list and emoji picker on Windows

### Rough

- Every dialog and popover Excord lays out itself was refused on Windows and Linux: the
  presenter had a native ContentDialog and MenuFlyout for a confirmation and a menu, and
  nothing for a box with its own content. The quick switcher, the profile, the member
  list and both pickers did not open. Fixed in the presenter (LLP 1115.004 D10.7): a modal
  dialog is centred over a scrim, a popover is placed against its invoker by
  `position-area`, and the page underneath is inert.
- Android still refuses them (QUEUE.md); the Draft in LLP 1115.001 D10 names what it owes.
- A field in one of these boxes has no edit menu yet, and the page's scrollbars paint over
  the scrim (QUEUE.md).

### Lean in

- Nothing in Excord changed: the same `showModal`, `popover` and `position-area` that
  place the boxes on the web and iPhone place them here.

### Needed

- dialogs and popovers: Fluent's metrics (ContentDialog's 8 px corner and 24 px padding,
  Flyout's Acrylic) wait on D12's Fluent column; they wear the neutral look until then.

### Checkpoints

- Windows, with the list beside the chat: the switcher opens centred over a dimmed window
  with its field focused, and the driver's `back` closes it. The emoji popover opens above
  its button, inside the window.
