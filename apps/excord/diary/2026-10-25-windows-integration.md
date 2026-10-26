surfaces: windows
agent: Codex (GPT-6)
task: combine native interaction fixes with newer chat features

### Rough

- Combining concurrent composer changes took about an hour. Both implementations
  changed keyboard handling and the transcript hierarchy. The merge needed to keep
  pending-edit isolation, loading failures and large-text constraints alongside
  suggestions, image viewing and attachments. Splitting the directory into its own
  component kept the main file within the source limit.

### Lean in

- Shared components let the newer transcript features use the same Windows text,
  image, scrollbar and menu fixes without app-specific native code.
- The fixture exercises message editing, failed sends and history through the real
  protocol client, making it useful for checking overlapping changes.

### Needed

- images: provided; combined remote downloading with Windows image decoding.
- keyboard: provided; retained composition handling and isolated pending edits.
- accessibility: provided; large-text menus now scroll and reveal keyboard focus.

### Checkpoints

- editing app.contract / app.ts: rough; overlapping composer changes, above.
- Windows release build: smooth after resolving the merge.
- driving the Windows app: smooth for images, the image viewer, menus, typing,
  retrying a failed send and Up/Escape editing. The existing local protocol fixture
  predates the newer member and avatar data, so these checks do not qualify them.
