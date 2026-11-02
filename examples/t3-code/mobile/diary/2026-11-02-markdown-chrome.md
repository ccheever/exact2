surfaces: ios
agent: Codex gpt-6-astra
task: match thread Markdown rule colors to the pinned feed

### Rough

The thread Markdown contract used each code fill as its own card border and
used a translucent generic border for tables. The pinned feed uses the opaque
`md-hr` role for code-card borders, code-card separators, and table rules.

### Lean in

The thread color shape now carries the generated `md-hr` role. Code cards and
tables use it for every outer and internal rule while retaining their existing
fills, widths, and overflow behavior.

### Needed

Native light and dark Markdown captures still need an owned iOS device drive.
This change makes no native visual claim without that drive.

### Checkpoints

- Thread, design, and scroll-focused tests pass: 109 tests and 715 assertions.
- `git diff --check` passes.
