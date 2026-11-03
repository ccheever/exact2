surfaces: ios
agent: Codex gpt-6-astra
task: match the older-activity control to the pinned mobile feed

### Rough

The history header's older-activity control used the primary color for its
label, muted tint for its chevron and spinner, a 5.25-point row gap, and no
loading opacity. The pinned feed uses foreground label text, the primary tint
for both controls, a 7-point gap, and 50% opacity while loading.

### Lean in

The control now receives a dedicated 60% border role, uses the upstream text
and control colors, spaces its row by 7 points, and applies 50% opacity while
the request is in flight. The existing single-control retry and error layout
remain unchanged.

### Needed

Native light, dark, loading, and error screenshots still need an owned iOS
device drive. This change makes no native visual claim without that drive.

### Checkpoints

- The focused history suite passes 33 tests and 224 assertions.
- The design suite passes 21 tests and 183 assertions, including the alpha
  role for light and dark themes.
- `git diff --check` passes.
