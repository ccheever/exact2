surfaces: ios
agent: Claude Opus 5.5
task: Home's rows run under its header with UIKit's scroll edge effect, as the thread's do

### Rough

Home's list stopped in a hard line under the bar while the thread's ran
under it. Two things kept it out, neither visible in the contract: the
zero-size `t3-home-swipe-events` view sat in flow above the list, and an
in-flow box above a route's scroller keeps it from the band under the bar
(`ControlBars.bars`); and the list's `overflow-x="hidden"` made its node clip,
so a banded scroll view would still have been cut at the node's top. Found by
reading frames in the running app with lldb: Home's route content started at
y=82, the thread's at 0.

On the iPhone Duo (951 pt, split layout) two more things kept the bar's edge
effect short of the right rail. Home reserved a 304 pt sidebar that the native
workspace never drew (lldb read the container's `configuration` as nil; why it
never arrived is not found), so Home's list ended 220 pt before the rail. The thread feed's column stopped at the rail
(`right=threadTrailingReservation`), and UIKit draws the effect only as wide as
the scroll view.

### Lean in

The swipe events view is now `position="absolute"`: it has no size, so
nothing moves. In exact2, a scroller's node no longer clips while its scroll
view is banded past it (`NodeView.clipsOwnBox`); the scroll view still clips
its content at the same sides, so Home keeps clipping its incidental sideways
overflow.

Home now shows no sidebar in split layout (its own list fills the width), and
the thread feed reaches the screen edge with the rail's width as right padding,
so its rows stay clear of the rail.

### Needed

A way to see why a route's scroller did or did not reach under its bar
without a debugger: the agent's `layout` could name the band and the node
that refused it.
