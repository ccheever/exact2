surfaces: shared Contract
agent: Codex
task: use the selected boolean operator spelling

### Rough
None in the source rewrite.

### Lean in
The parser identifies operator spans, including template expressions.

### Needed
none

### Checkpoints
- Boolean operators were migrated by parser spans; comments and string text were preserved. App verification is recorded in the simplification RFC.
