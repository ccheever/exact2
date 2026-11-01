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
The existing projection emits dividers only between adjacent rows, so no
first-row or final-row separator was added. List-level horizontal clipping was
rejected because Markdown and code blocks own their own horizontal scrollers.

### Needed

A fresh supported iOS drive is still needed to measure scroll motion, dynamic
row anchoring, Duo rail reachability and frame pacing. No framework defect is
claimed; an issue or framework PR requires a reproduced failure in an isolated
worktree first.

### Checkpoints

The focused Home/thread suite passes 152 tests and 1,112 assertions. Contract
compilation, strict app TypeScript, boot and all ten caps categories pass.
The first source commit was `98e425ba72`; the row-root correction is a follow-up
source unit. No native build or performance claim is attached to this work.
