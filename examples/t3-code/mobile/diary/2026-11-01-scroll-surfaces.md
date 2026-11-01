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
Home's compact-row divider now sits beside the pressable row content in the
noninteractive row container. It keeps the authored margin and adjacent-row
predicate without inheriting the button's swipe translation or clipping box.

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

The follow-up keeps the same app ownership boundary. Home scrollports and swipe
row roots now use border-box and a shrinkable inline size, with an 88-point
virtualization estimate for the mixed card/slim feed. Thread end-follow no longer
asks the host to animate every streamed correction; history anchor restores remain
explicitly instant. This removes repeated retargeting while a response grows and
keeps authored padding out of the measured scroll width. Home dividers remain
projection-owned and only occur between adjacent rows; the fallback work row no
longer adds a bottom rule of its own. Focused Home/thread tests,
the full mobile suite, Contract build, strict TypeScript, caps and boot pass.
The local Family Reference iPhone 16 was unavailable because another workspace
held its device session; no runtime or framework defect claim is made.

The final source pass also removes native button chrome from authored Home and
thread controls, hides the Home scrollbar, and dismisses the keyboard when a
Home drag begins, matching the reference list behavior without changing row
keys or adding a scroll surface. The combined Home projection now decides the
first Unsent section marker, so draft-only and pending-only lists have no
detached leading rule and no row emits a trailing rule after the last item.
Focused tests pass 202 tests with 1,481 assertions, the full mobile suite passes
4,240 tests with 2 optional bridge skips and 37,681 assertions, and the normal
no-install iOS build passes with strict deep codesign (92 bundle files;
executable SHA-256
`13355523775b4dc20440bb6cf13131237ae29e71beed5d50bb501a4fe2061c1b`).
The only runtime attempt was refused because the Duo was owned by another
workspace session; no framework defect or performance claim is made until an
exclusive device window is available.

The nested work-detail scroll now has a shrinkable border-box, contains edge
flings, and hides its incidental scrollbar. Its authored padding stays inside
the 210-point detail viewport, and a detail fling cannot chain into the
transcript. The focused source tests, full mobile suite, Contract build,
strict TypeScript, caps, boot and normal no-install native build pass. The
fresh A44 Duo was still occupied when the built artifact was ready, so live
frame pacing and divider placement remain unqualified.

The follow-up gives Home, the transcript, and nested work detail an explicit
vertical touch axis so UIKit can reject competing horizontal pans before a
scroll begins. Home's authored separator strokes are border-boxed and guarded
against a last-row draw in the Contract as well as the projection. The focused
scroll and Home suite passes; a clean device drive is still required before
calling the motion buttery or confirming the rendered edge lines.

The final source pass has a normal no-install iOS build with all 3,289 recorded
inputs present and unchanged, a 92-file bundle inventory, and strict deep
signature verification. The host's iOS tests already cover `pan-y` admission
and native swipe precedence; no framework failure reproduced. Every local Duo
target was occupied by another session, so this unit makes no runtime frame
pacing or rendered-divider claim.

The follow-up removes default native button chrome from Home swipe actions and
Markdown code-copy controls. Code and table scrollports now own the horizontal
touch axis, contain edge overscroll, hide incidental scrollbars and include
their authored padding in the measured border box. Focused source tests pass;
the normal source build succeeds. A runtime drive could not start because both
available local simulators were owned by another workspace, so rendered motion,
divider placement and any framework issue remain unqualified.

The next source review removes `touch-action="pan-x"` from the Markdown code and
table scrollports. UIKit intersects that restriction with the transcript feed,
so a vertical drag beginning over a code block could be rejected before the
feed saw it. The scrollports still own horizontal overflow, contain edge
overscroll and hide incidental indicators, matching the existing file-preview
pattern. Focused and full mobile tests pass; both local Duo targets were still
owned by another workspace, so live frame pacing and rendered line placement
remain unqualified.
