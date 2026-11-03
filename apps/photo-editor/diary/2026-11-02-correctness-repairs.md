surfaces: data-module test harness, web
agent: Codex (GPT-6)
task: Give concurrent photo saves independent temporary files.

### Rough

Concurrent saves used one temporary path. Each save now owns a fresh path and removes its own file afterward.

### Lean in

The app's data-module test harness makes the failure reproducible without a live service or device.

### Needed

- photos: provided
- files: provided

### Checkpoints

- Editing app data code: smooth. Concurrent save and cleanup regressions passed. No PhotoKit operation or device save was performed.
- Web build and drive: rotating and resetting the bundled image worked. Both light and dark preference tests passed. The local print backend was absent; the initial screen check found no issues.
