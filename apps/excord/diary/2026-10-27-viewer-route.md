surfaces: ios, web, macos
agent: Claude Code (Claude Opus 5.5)
task: make the image viewer a full-screen route that zooms out of its thumbnail

### Rough

- The first iOS build of the route showed white bands above and below the black viewer:
  the host keeps a full-screen route's content clear of the status bar and the home
  indicator, and a child at `inset=0` does not reach past that. The route's own column
  now carries the black backdrop, which the host paints to the edges
  (docs/agent-pitfalls.md).
- That background is painted once on iOS, so the drag-down fade, now an animation of the
  route's background, shows on the web (to the host's grey under the route) and not on
  iOS, where the picture moves over black and zooms back into its thumbnail on release
  (QUEUE.md).
- On the Mac a split's full-screen route is drawn in the list's column, beside the chat;
  origin/main's root overlay did the same. A host fix (QUEUE.md).
- `keyframes` refuse a colour role (`-exact-background`); the fade names `#000`.
- Review found the web's history the hard part. Nested under its channel, a picture's
  route reloaded as three entries with one written to history, and dropping it pushed the
  channel after the picture's address: Back looped. The route is now the list's child, so
  a reload's stray picture is replaced by its channel in its own history entry. A channel
  opened from over the picture (`replace(back(nav), …)`) left the old channel's address:
  the web's history mirror went back to the entry and did not rewrite its URL; fixed in
  `host/web/navigation.js`. Forward to a picture is refused, and the Close button is the
  route's `navigationBack` control, so the browser's Back presses it.
- The reload's replace left two entries at the channel's address, and the history mirror
  refused a pop that changed nothing, so Back stayed there; it now accepts an entry at the
  address already shown that the page did not write, or that holds the visit shown (`host/web/navigation.js`, LLP 1038). The agent cannot drive this:
  a drive's `reload` leaves no history behind it, and `history -1` is no test step. A
  Chromium page over the mirror alone showed it stuck before and reaching `/` after.
- Closing the picture on a lock made the transcript count as read behind the lock screen;
  `reading` now excludes the lock.
- The agent's iOS drives present without animation, and `screenshot over` refuses
  `--timing platform`, so the zoom was seen in `simctl io screenshot`s taken in a loop
  during a `--timing platform` drive.

### Lean in

- `navigationSource` naming the thumbnail's `id`, with the viewer's picture taking the
  same `id`, made the photo zoom out of its thumbnail and back on iOS with no host code.
- A route made the browser's Back, Escape, UIKit's inert page under the viewer and its
  status-bar inset come with nothing written for them.
- The picture's address (`/c/:channel/photo/:photo`) carries the channel's parameter, so
  the launch restore and the screen kept for relaunch read one derive (`screen`) instead
  of learning a second route.

### Needed

- navigation: provided; a full-screen route over a Mac split is drawn in the list's
  column, and the Mac's Back (⌘[) does not leave one (it is not a push).
- safe areas: provided by the route on iOS; a backdrop that reaches the screen's edges
  and fades needs LLP 1123 D4.

### Checkpoints

- iOS: Close sits below the status bar; the picture zooms out of its thumbnail and back on
  Close and on a drag down; at fit a short drag springs back; zoomed, a drag pans and does
  not close.
- web: Escape, Back and Close close it and leave the channel; a reload over it shows the
  channel, at the channel's address; a channel picked in the switcher over the viewer
  replaces both, at its own address; a Forward back to the picture is refused, and Back
  from there reaches the list.
- macOS: Escape and Close close it.
- web contract run: 79 of 79; iOS: 76 of 79, the three that fail on origin/main.
