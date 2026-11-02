surfaces: data-module test harness, web
agent: Codex (GPT-6)
task: Cancel a send requested before its background job is created.

### Rough

Stopping a send during job creation lost the cancellation. The app now retains the stop request until the job identifier arrives.

### Lean in

The app's data-module test harness makes the failure reproducible without a live service or device.

### Needed

- background work: provided

### Checkpoints

- Editing app data code: smooth. The held send-job regression passed, including cancellation before the job identifier arrives.
- Web build and drive: the demo conversation opened and Back returned to the inbox. The screen checker reported seven swipe-row overflows and three contrast failures. No message was sent.
