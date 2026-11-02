surfaces: data-module test harness
agent: Codex (GPT-6)
task: Preserve a stop request until a background job returns its identifier.

### Rough

A stop could arrive before job creation returned its identifier, leaving the new job running. The app now remembers the stop and cancels the returned job.

### Lean in

The app's data-module test harness makes the failure reproducible without a live service or device.

### Needed

- background work: provided

### Checkpoints

- Editing app data code: smooth. The held job-creation regression passed; stopping during creation prevents the returned job from continuing.
