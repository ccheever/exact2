surfaces: ios
agent: Codex gpt-6-astra
task: reuse unchanged conversation Markdown during streaming

### Rough

The first native build could not find the offline `wat` dependency. A root
`cargo fetch --locked` with `CARGO_NET_GIT_FETCH_WITH_CLI=true` filled the cache;
the retry built and signed the simulator bundle. The full mobile baseline had one missing
compiler failure. After building the compiler, that fixture timed out at its
existing five-second limit while other cold builds were active. It needs a
quiet rerun. No timeout or check was relaxed.

The device tools could not discover the installed iPhone Duo simulator, and the
EAS host reported no selected project. Native acceptance is pending the
coordinator's device setup. There are no screenshot, interaction or timing
results for this change.

### Lean in

The existing client fixtures exercise production transcript projection without
a provider. They verify content, Copy ownership, reasoning disclosure and
previous-snapshot immutability while the Markdown cache reuses unchanged blocks.
An attachment visibility scan now skips Markdown parsing and leaves the visible
route's cache intact.

### Needed

- scrolling: provided by the keyed native list; prepend and streaming anchor
  behavior still need device acceptance.
- caching: by hand; the new snapshot cache retains at most 128 entries,
  512 Ki accounted UTF-16 units and 32,768 block/token objects. These bounds
  cover the cache, not total transcript or native memory.

### Checkpoints

- editing app.ts: smooth; strict ES2024 TypeScript checking passes.
- focused conversation tests: smooth; 136 tests pass, including bounded reuse,
  edited content, scope invalidation and full oversized-message rendering.
- iOS baseline build: rough; dependency recovery above. The development bundle
  built and signed with `--no-install`. Its module roster probe passed on a
  simulator; the app was not installed or driven, and the bundle predates this
  cache change.
- driving the app: pending device availability. Release comparisons, streaming
  and Stop, automatic bounded history, and Duo acceptance remain outstanding.
