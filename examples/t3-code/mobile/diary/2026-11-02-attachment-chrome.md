surfaces: ios
agent: Codex gpt-6-astra
task: match assistant image attachment chrome to the pinned feed

### Rough

The shared image attachment view applied the user-bubble geometry to assistant
images without the reference feed's separation or backing surface. The pinned
feed adds 5.25 points above assistant images and uses the `subtle-strong` role.

### Lean in

The thread color shape now carries `subtle-strong`. Assistant images use the
reference top spacing and backing while user-bubble images retain their
existing transparent backing and radius.

### Needed

Native light and dark image captures still need an owned iOS device drive. This
change makes no native visual claim without that drive.

### Checkpoints

- Thread, design, and scroll-focused tests pass: 109 tests and 711 assertions.
- `git diff --check` passes.
