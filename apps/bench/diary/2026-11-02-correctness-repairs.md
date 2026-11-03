surfaces: data-module test harness, web
agent: Codex (GPT-6)
task: Reject invalid DFUse sector sizes.

### Rough

A zero sector size left firmware parsing in a nonterminating loop. Invalid and oversized sector sizes now fail before allocation or iteration.

### Lean in

The app's data-module test harness makes the failure reproducible without a live service or device.

### Needed

- USB: provided

### Checkpoints

- Editing app data code: smooth. DFUse parsing regressions passed, including zero and oversized sectors. No USB device was flashed.
- Web build and drive: the firmware and monitor tabs opened. The initial screen check found no issues. No device was connected.
