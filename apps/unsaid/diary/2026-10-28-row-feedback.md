surfaces: web
agent: Codex
task: verify scrolling and pressed-row feedback

### Rough

A button nested inside the virtual row's column received feedback inside the column's platform padding. The row root now owns the numbered row's action, so its background fills the cell. The first two rows disable that action while retaining their nested scrollers. Conditional template roots were refused because a virtualized list needs one element flow root; the diagnostic explained the restriction.

Safari's bounce offsets triggered corrective writes from the virtualizer. The web host now bounds its logical vertical offset while leaving native overscroll to the browser.

### Lean in

The existing list defaults supply row padding and feedback when the authored row root owns its action.

### Needed

- scrolling: provided, rough (see Rough)
- press feedback: provided, rough (see Rough)

### Checkpoints

- web build: smooth
- physical Safari feedback: rough (see Rough); after the fixes, the user retested the public preview and reported "Works well"
