# Code review: the interim conform marker for apps whose wasm reference binds no packages (photo-editor, recorder, markdown), round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `275683996c`.
- **Method:** one brief (sha256 `330411408f929702091a4067cb442ab70ac477cf54e54bb69cfb0ace1c300e93`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The marker is exactly `photo-editor`, `recorder`, and `markdown`, each with the reason `reference does not bind packages (LLP 1071 retirement)`, on a frozen object whose prototype is `Object.prototype` (`host/web-js/conform-marked.mjs:12-16`). `markUnbound` moves a failure only when `Object.hasOwn(marker, f.target)` and `f.difference === true` (`conform-marked.mjs:26-27`), so `synthetic-recorder`, `markdown-stress`, and `toString` stay failures. `markUnbound` runs only when there is no `--browser` (`host/web-js/conform.mjs:924`). Firefox and WebKit runs in `scripts/async.mjs:174-175` pass `--browser`, so they never take that path. Linux comparisons never set the flag: state and tree (`conform.mjs:438`), layout (`conform.mjs:451`), clock (`conform.mjs:517`), bound refusals and sound (`conform.mjs:534-535`).

Every wasm-versus-JS difference kind is flagged and then moved: state (`conform.mjs:426`), tree (`conform.mjs:429`), layout (`conform.mjs:447`), paint (`conform.mjs:459`), screenshot pixels (`conform.mjs:488`), bound refusals (`conform.mjs:530`), sound lines (`conform.mjs:531`). These three apps have no `conformance/<app>.steps`, so the scripted refusal path does not run for them. What stays a failure is a JS `state()` throw (`conform.mjs:422`), a screenshot throw (the drive `catch` at `conform.mjs:596-597`), a step the reference could not take or that one page refused (`conform.mjs:558`, `conform.mjs:563`, `conform.mjs:566`, `conform.mjs:585`), an unsettled clock (`conform.mjs:523`), app tests (`conform.mjs:627-632`), and direct build, open, harness, and net pushes (`conform.mjs:314`, `conform.mjs:330`, `conform.mjs:857`, `conform.mjs:911`, `conform.mjs:922`). A flagged difference cannot be one of those: the flag is only passed from the seven comparison sites above.

Counts are written after the move. `report.json` gets `marked.<target>` as `{reason, differences, steps, lines}` (`conform.mjs:931`). `report.md` adds `## Marked: …` with the count and the first lines, and the failures cell reads `N (+M marked)` (`conform.mjs:941`, `conform.mjs:945-951`). Stdout prints a `MARKED` line per target and a summary that includes the marked total (`conform.mjs:971-972`). `--strict` exits 1 only when `report.failures.length` is still non-zero after the move, and it prints `FAIL` only for those rows (`conform.mjs:973-979`).

The first test locks the three names, the exact reason on the constant and on every value, the freeze, and the prototype (`host/web-js/conform.test.mjs:155-162`). The second feeds realistic records: state, tree, and screenshot diffs are counted onto `recorder`; a stale open, a one-page refusal, a crash, a wasm build, an app-test failure, `caltrain`, `synthetic-recorder`, and `toString` all remain in `failures` (`conform.test.mjs:165-186`). Layout, paint, bound refusals, sound, and Linux are the same flag check, not a second classifier.

The async lane does not need a parser change. Its wasm run is `conform.mjs` with `--strict` and no `--browser` (`scripts/async.mjs:166`), and it files a failure only from a `FAIL` line or a non-zero exit (`scripts/async.mjs:293-305`). Marked diffs produce neither. A build, crash, one-sided refusal, app test, or Linux miss on those three still prints `FAIL` and exits 1. `conform-firefox` and `conform-webkit` stay strict cross-browser runs.

APPROVE
