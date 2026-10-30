# Code review: the JS agent tree's inline image type, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `e9143679ed`.
- **Method:** one brief (sha256 `87037bfdef1e0781d0f2a4dc7466558a54f50f526f0991899c63f52c64b3b824`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **The required reader inventory is incomplete.** The commit omits the propagation into layout at [scripts/agent-inspect.mjs:165](scripts/agent-inspect.mjs:165), the comparison grouping at [scripts/agent-compare.mjs:312](scripts/agent-compare.mjs:312), and the Image filter at [scripts/stage12-named.mjs:77](scripts/stage12-named.mjs:77). The latter matters to the audit because it selects regions eligible for image-noise tolerance; its current caller is restricted to wasm at [scripts/stage12-gate.mjs:211](scripts/stage12-gate.mjs:211), so this change does not affect it.

   Other omitted readers include [scripts/verify.mjs:497](scripts/verify.mjs:497) and its lines 605/738, [scripts/smoke.mjs:541](scripts/smoke.mjs:541) and its lines 580/690/739/1464, and [scripts/design-tool.test.mjs:160](scripts/design-tool.test.mjs:160). These select other node kinds or actual text labels; none requires an inline image to remain Text. Extend the commit’s inventory with these results to satisfy approved condition (1). This is an audit-completeness blocker, not a discovered runtime regression.

**Non-blocking**

1. **The new regression test is excluded from the async test lane.** [scripts/async.mjs:113](scripts/async.mjs:113) excludes the entire `document-agent.test.mjs` file. Conform still covers the classification regression, so protection exists, but the new source/label assertions will not run there. Move this test into an existing included browser-test file or explicitly include this case.

2. **Assert that the image has no `props.text`.** [host/web-js/document-agent.test.mjs:210](host/web-js/document-agent.test.mjs:210) checks type, label and source, but misses this changed behavior. Add an absence assertion: [host/web-js/conform.mjs:192](host/web-js/conform.mjs:192) normalizes missing text and `""` identically, so conform cannot catch that distinction.

The fix itself is correct and appropriately narrow:

- [contract/lower/src/nodes.rs:802](contract/lower/src/nodes.rs:802) permits only text/time runs and direct images in these paragraphs. The kernel independently rejects other child types at [kernel/src/txn.rs:427](kernel/src/txn.rs:427). Video, audio, canvas, iframe and controls therefore need no additional exception.
- A symbol rendered through `image` remains an IMG; actual SVG nodes are classified before `run(el)`. Inline text links must remain Text despite their A tag, so moving all of `TYPES` ahead of `run` would be wrong.
- For the fixture, [host/web-js/agent.js:97](host/web-js/agent.js:97) preserves `imageSource` and `accessibilityLabel`, while line 101 now omits the spurious text property. The runner reports the kernel’s Image type directly at [runner/src/agent/tree.rs:77](runner/src/agent/tree.rs:77).
- The commit’s exclusions for agent-ax, check.js and the design app are accurate: they use AX/DOM facts or the separate inspect table’s type, not this classifier.

The test is real: it builds a page and reads its actual agent tree. Source inspection confirms the parent would return Text and fail its Image assertion. I did not execute tests/builds or independently verify historical red-first results, zero/new conform failures, or caps.
