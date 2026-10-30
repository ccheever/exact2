surfaces: semantic harness
agent: Codex (GPT-6 Astra)
task: state existing proof properties as authored test blocks

### Rough

Typing initially delivered change only, so the note input handler was not reached. The harness now selects input when present, on both evaluators.

### Lean in

The same property lines check the runner's and Lean's observations without changing the app.

### Needed

none

### Checkpoints

- Authored tests and property verification: The authored sequence and explore script agree with Lean; all five properties held.
- Suggestions kept: of the 8 blocks the search suggested, a four-family panel judging as the app's author kept 2, now in the test file beside the proved properties' blocks; the rest are sample artifacts or what the type says.
