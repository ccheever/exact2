# Code review: the JS agent tree's inline image type, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `5903dfc60d`.
- **Method:** one brief (sha256 `2623aa81673a4ba3034c3b1b2ccf867a70e7e5e43f5acfb1ab86057fbf31a459`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited, except that the brief's message file is named as the squash message (the landed commit's message).

**Blocking**

none. I found no reader that requires an inline image to remain `Text`.

The test concerns are resolved: [inline-image-agent.test.mjs:39](host/web-js/inline-image-agent.test.mjs:39) asserts `Image`, line 41 asserts missing `props.text`, and [async.mjs:115](scripts/async.mjs:115) includes the file without excluding it. Source inspection confirms the parent’s `run(el) ? 'Text'` branch would fail the assertion. `document-agent.test.mjs` matches the parent. No tests or builds were run.

**Non-blocking**

1. The squash message (the landed commit's message), line 15 — The inventory is still not literally exhaustive. It omits [agent-test.mjs:90](scripts/agent-test.mjs:90) (type presence), [agent-test.mjs:135](scripts/agent-test.mjs:135) (diagnostic label), and [design-tool-canvas.test.mjs:103](scripts/design-tool-canvas.test.mjs:103) (a specific button’s `Pressable` assertion). Add these entries; all are unaffected. Also clarify that [inspect-dev.js:235](host/web-js/inspect-dev.js:235) gets its type from the compiler’s inspect table, independently of the agent tree.

2. The squash message (the landed commit's message), line 29 — “the same join” needs qualification. If the image supplies the only descendant `props.text`, removing its empty string changes `textOf` from `""` to `undefined` ([agent-test.mjs:31](scripts/agent-test.mjs:31)). Say the joined text stays unchanged when another text run remains. This matches the intended runner parity and requires no code change.

APPROVE
