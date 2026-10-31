surfaces: ios
agent: Codex gpt-6-astra
task: reuse conversation Markdown and present tables without losing source content

### Rough

The first native build could not find the offline `wat` dependency. A root
`cargo fetch --locked` with `CARGO_NET_GIT_FETCH_WITH_CLI=true` filled the cache;
the retry built and signed the simulator bundle. The full mobile baseline had one missing
compiler failure. After building the compiler, that fixture timed out at its
existing five-second limit while other cold builds were active. It needs a
quiet rerun. No timeout or check was relaxed.

The final normal development bundle at `a9981950a3` rendered six complete
synthetic provider turns on an actual iPhone Duo. A completed table still
displayed literal pipes after reopening the conversation. This path passed the
table to Exact's inline Markdown text renderer. That renderer uses monospace
table source; the app must supply a separate table view.

The Duo's outer rail overlapped Send while the keyboard was visible. Back and
reopen preserved the draft and exposed a working Send. A later device-tool
snapshot timed out after its prescribed recovery. The owned session was closed
without shutdown, preserving six completed turns and the seventh unsent draft.
History beyond the initial ten-turn page remains unqualified.

### Lean in

The existing client fixtures exercise production transcript projection without
a provider. They verify content, Copy ownership, reasoning disclosure and
previous-snapshot immutability while the Markdown cache reuses unchanged blocks.
An attachment visibility scan now skips Markdown parsing and leaves the visible
route's cache intact.

Conversation tables consume the existing generated GFM parser without changing
it. Supported cell runs retain decoded text, code, bold and italic content.
Unsupported content and ragged rows keep their complete source. Message Copy
still uses the original body. Table rows, cells, runs and arrays count against
the presentation cache's existing object and text budgets.

### Needed

- scrolling: provided by the keyed native list; prepend and streaming anchor
  behavior still need device acceptance.
- caching: by hand; the new snapshot cache retains at most 128 entries,
  512 Ki accounted UTF-16 units and 32,768 accounted objects. These bounds
  cover the cache, not total transcript or native memory.
- tables: by hand; one shared-column grid within a horizontal scroller.
  The frozen reference `80a2351fcb909034e65a512dfabeb3cdfd367762` uses native
  measured columns with a 60-point minimum and a 300-point content cap plus
  padding. The Contract grid is provisional. Width, selection, clipping and
  animation parity need paired native acceptance.

### Checkpoints

- editing app.ts: smooth; strict ES2024 TypeScript checking passes.
- focused conversation tests: smooth; 136 tests pass, including bounded reuse,
  edited content, scope invalidation and full oversized-message rendering.
- iOS baseline build: rough; dependency recovery above. The development bundle
  built and signed with `--no-install`. Its module roster probe passed on a
  simulator; the app was not installed or driven, and the bundle predates this
  cache change.
- root checks at `a9981950a3`: smooth; all required categories pass. The signed
  normal bundle's 3,289 source hashes match. These receipts predate the table
  change and are not Release performance evidence.
- table regression: smooth; the test's 385-byte literal equals the completed
  Duo response. It failed before table projection and passes after it. The
  focused conversation and outbox suite passes 152 tests and 1,135 assertions.
  Strict application
  TypeScript and full Contract compilation pass.
- table stress fixture: rough; 8,201 rows exceeded the existing five-second
  limit. A denser 1,301-row, eight-column fixture still exceeds the object
  budget and verifies complete uncached output without changing the timeout.
- driving tables: pending a new native artifact and device window. Release
  comparisons, automatic bounded history and full Duo acceptance remain open.
