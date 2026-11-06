surfaces: ios
agent: Codex gpt-6-astra
task: preserve source subagent links in the mobile transcript

### Rough

The shared transcript already identified a notification's child thread, but the mobile activity adapter discarded its target. A person could see the completed subagent card without a way to open that child thread.

### Lean in

Mobile activities now retain the child thread ID and label it as an Open subagent or Open chat action. The work-row Contract exposes that action, and the existing root route replaces the current thread through the normal navigation owner.

### Needed

Native validation still needs a populated parent thread containing a subagent notification. This source correction does not claim native pixel or route animation parity.

### Checkpoints

- Focused work and transcript tests cover direct subagent activities and notification projection.
- Contract generation, the full mobile suite and the iOS build remain to be run after this edit.
