surfaces: ios, android, macos, web
agent: Claude Code (Claude Opus 5.5)
task: a message's link opens over the conversation (LLP 1133.024 stage 4, `@exact/in-app-browser`)

### Rough

- `app.contract` was already at the line cap, so the browser's resource, mutation and action
  went into a members file (`browse.contract`), as the mention light had.
- A drive script under `packages/<name>/test/` is an input of the app's build, so each edit to
  it made the iOS, Mac and web builds stale and the driver refused them until they were rebuilt.
- `open()` from `scripts/agent.mjs` starts no app server; the CLI does, through
  `verify.mjs`'s `driveServer`. A script that drives Excord calls it first, or the app waits on
  "Loading channels" for a server nobody started.
- `s.state('browser')` reads a view named `browser`; a resource by name is `stateOp(s,
  ['browser'])` from `agent-inspect.mjs`, as the CLI's `state browser` reads it.
- On Android the build reaches the fixture at the Mac's loopback, which the emulator's is not:
  the drive runs `adb reverse` for the server's port.
- A module's `context.log` on Android goes to logcat, not the agent's journal.

### Lean in

- The `browser` ticket made the drive the same on iOS and Android: tap the title, find the
  ticket in `pending`, answer it with `{ choice }`.

### Needed

- in-app browser: built on iOS, Android, macOS and the web; Windows comes with LLP 1133.024
  stage 6.

### Checkpoints

- iOS simulator: an embed's title holds a `browser` ticket naming the page; `dismissed`
  answers it, and the journal names the page's origin, not its path.
- Android emulator with Chrome: the same, answered `opened`.
- macOS and the web: `overApp` is false and the title follows its link.
