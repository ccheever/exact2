# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `c6382d7a3`.
- **Method:** one brief (sha256 `ae3d5112a631b2360b4b31b59be8374c74d62cf20225beded3f7819454bfc42e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve with nonblocking lows (3 low).
- **Disposition:** taken, but the away-build low in part, as in round 2. This was the last round.
  - The stub's off events carry `to` as the server's do, and so does the contract test's takeover.
  - docs/tamber-protocol.md's `selected` report lists `inspect`, its command's number (*1.5*).
  - tamber-inspect.test accepts a selection naming the current command.
  - The page drops the node it selected under a command when another command comes (another owner, or off and on), so a later refresh does not report a click the server refused (Grok's low).
  - In part: after a missed poll, an away build this tool owns shows only when it registers again.

---

No High or Medium findings in the updated page/server path. The earlier correctness fixes hold: ownership changes publish in order, replaced reports are suppressed, the page discards 409s without blocking its queue, and `to: null` preserves staleness across stop/start batching. The 1.5 bump is consistent; remaining 1.4 references are historical.

- **Low — Missed polls still forget away owners.** [apps/design-tool/app.ts:1115](/tmp/rv-dt-ievent-r3/apps/design-tool/app.ts:1115). If every owned build is disconnected when a poll misses events, clearing `inspectOn` followed by hello cannot restore ownership: hello lists only connected builds. Stop disappears until reconnection, while continued polling keeps ownership alive. This is the acknowledged partial disposition. **Fix:** include disconnected ownership in the resync snapshot and test that Stop remains available.

- **Low — The stub still misrepresents takeovers.** [apps/design-tool/stub/tamber-stub.ts:110](/tmp/rv-dt-ievent-r3/apps/design-tool/stub/tamber-stub.ts:110), [app.test.contract:948](/tmp/rv-dt-ievent-r3/apps/design-tool/app.test.contract:948). Both omit `to` on takeover losses. An observing tool therefore treats these fixtures as actual stops and incorrectly stales its selection; the current test has no selection before takeover, so it cannot detect that distinction. **Fix:** emit production-shaped events and assert that an observer retains its selection across takeover but loses it across explicit off/on, including within one poll.

- **Low — The selection-report schema omits its new command number.** [docs/tamber-protocol.md:687](/tmp/rv-dt-ievent-r3/docs/tamber-protocol.md:687). A host following the documented non-null envelope sends no `inspect`; the document mentions that field only for exits. Such reports bypass the optional sequence comparison after takeover. **Fix:** document `inspect` on non-null reports and add a successful matching-sequence test alongside the rejected-sequence case.

Tests were read, not run, as requested. Replacement/eviction publication, ownership recovery after missed polls/new epochs, and Outline transitions still lack direct assertions.

**Verdict: approve with nonblocking Low findings; the acknowledged away-build limitation remains.**