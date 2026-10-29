surfaces: web
agent: Claude Code (Claude Opus 5.5), as iosfix
task: make another client's edit show in the tool's token grid quickly

### Rough
- A peer's edit took 4.5 s to reach the grid on a 267-token design (a
  copy of the Bluesky clone, five runs, two clients). The first guess was
  the long poll's stream not re-keying the read. Measuring it in Chrome
  (each `/__tamber` request timed by method) showed the read starting
  10 ms after the event. The whole delay was 267 one-token `resolve`s,
  queued one behind another at the design server, for one token's
  change. About an hour went on setting up the copy: a Cargo patch path,
  then the lock, then the `file:` package paths all pointed at another
  checkout. `bun exact.mjs update` and `lock` with `EXACT2=<checkout>`
  fixed the first two; the package paths were edited by hand.
- From reading session.rs (a plan folds each file's own text), I took a
  write's `changed` to miss a package's token derived from one the app's
  file set (the package's `reading.edge = reading.card` when the app sets
  `reading.card`), so the read again also resolves every token whose
  `describe` entry moved. A server test later showed `changed` does name
  it: the app's design is planned folded with its packages' (the synthetic
  file's params carry them). The entry comparison stays as belt and
  braces. Lesson: test a protocol claim against the server before building
  on it.
- `app.ts` sits at the 1,500-line cap, so the logic went into
  `reread.ts`, and `app.ts` has two more joined lines.

### Lean in
- `describe` carrying each token's folded values made "what moved" a
  plain comparison, with no dependency walk in the tool.
- The harness's `proxied` server let the test count every request by
  method and land a peer's write and a hand edit between two drive steps.

### Needed
- none

### Checkpoints
- web dev loop: smooth
- editing app.contract / app.ts: smooth
- driving the app (`exact.mjs agent`, `exact.mjs test`): smooth
