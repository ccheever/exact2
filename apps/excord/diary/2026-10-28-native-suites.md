surfaces: ios, macos
agent: Claude Code (Claude Opus 5.5)
task: Excord's contract suites on the iPhone simulator and the Mac

### Rough

- `members` and `typing` failed on the Mac and the iPhone: they tapped a channel the
  moment the gateway was ready, before the launch's own work had settled. A `clock
  settle` after the ready wait fixed both, and the typing tests' 5 s bound on that wait
  is now 10 s, as the other suites have.
- On the iPhone, the agent's `back` with the member sheet open popped the channel under
  it: the iOS host's `back` cancelled an alert or a popover, then went to the route,
  skipping a `dialog` shown as a sheet. It now closes the top dialog sheet as Escape
  does, as the Mac's does (`DialogSheets.agentCancel`).
- Then the next `back`, on a member's card, found no sheet. The card's action closes the
  member sheet and calls `showModal("profile")`, which on iOS retries every 0.1 s until
  the leaving sheet frees UIKit's presentation, and `clock settle` did not count those
  retries: it reported a fixed point with the card not yet shown. They count now
  (`MenuHost.waitingShows`).
- On the iPhone, the account alert's Close was refused as unavailable: the agent's tap
  first light-dismissed the painted account menu under the alert, and that menu held
  the button the alert came from. A touch on an open alert never reaches the page, so
  the agent's no longer dismisses through it.
- "retrying older history at the exact top" typed Home at the transcript, which on Apple
  hosts goes to the composer. It now drags to the top, as a phone does; on the Mac a
  drag selects text, so that test still fails there (QUEUE.md).

### Checkpoints

- ios: every suite passes (`app` 69, one skipped).
- macos: `members` and `typing` pass.
