surfaces: ios
agent: Codex gpt-6.1-sol
task: keep Home and thread scroll surfaces within their route columns

### Rough

Home and the conversation already use keyed virtualized lists. The reported
rail overlap and clipped-looking dividers came from route lists that did not
declare a shrinkable inline size and row dividers that extended to the edge.

### Lean in

Both lists and their virtualized row roots now declare `min-width=0`, allowing
the route column to own their width. Home dividers have explicit horizontal
margins in both card branches.
Home and thread feeds also explicitly use `appearance="none"`: these surfaces
author their own between-row dividers and message spacing, so the platform row
cell separator must not add lines above the first or below the last row. This
also removes the separator overlay work from these scrolling surfaces.
The thread feed opts into the host's smooth end-follow behavior. History
prepend corrections continue to use the root's instant keyed anchor restore;
only new content while already following the end is eased.
Home owns a horizontal swipe action, so its vertical list hides incidental
horizontal overflow and contains edge pans. The thread feed contains edge pans
while leaving its existing horizontal overflow behavior unchanged for the
Markdown and code scrollers it contains. Neither setting adds a second scroll
surface or changes row keys.
The app's scroll-facing snapshots now use sampled clocks instead of reading
`performanceNow()` in resource arguments. Idle views use the existing minute
sample, while an active thread uses its one-second agent sample for elapsed
work labels. This prevents Home, thread data, archive and related projections
from re-running every frame while a person scrolls; command timestamps and
native animation clocks keep their live timestamps.
The existing projection emits dividers only between adjacent rows, so no
first-row or final-row separator was added. List-level horizontal clipping was
rejected for the thread because Markdown and code blocks own their own
horizontal scrollers; Home's list has no authored horizontal content and now
clips only incidental sideways overflow.

### Needed

A fresh supported iOS drive is still needed to measure scroll motion, dynamic
row anchoring, Duo rail reachability and frame pacing. No framework defect is
claimed; an issue or framework PR requires a reproduced failure in an isolated
worktree first.

### Checkpoints

The focused Home/thread suite passes 223 tests and 1,595 assertions. Contract
compilation, strict app TypeScript, boot and all ten caps categories pass.
The first source commit was `98e425ba72`; the row-root correction is a follow-up
source unit. No native build or performance claim is attached to this work.

The sampled-clock correction passes its source-focused run (138 tests, 854
assertions), the full mobile suite (4,238 passed, 2 optional skips, 37,663
assertions), Contract compilation, strict TypeScript, caps, boot, and a normal
no-install iOS build. A runtime scroll drive remains pending because the
assigned Duo is held by another device session; no framework defect is claimed.
