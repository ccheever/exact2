# Code review: the JS agent tree's inline image type, round 2 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `5903dfc60d`.
- **Method:** one brief (sha256 `2623aa81673a4ba3034c3b1b2ccf867a70e7e5e43f5acfb1ab86057fbf31a459`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The three round-1 items are resolved. The type change matches the kernel (`NodeType::Image` for an image whose parent is a paragraph), and every reader that branches on an inline image being `Text` is in the squash message and does not keep that assumption.

**Blocking**

none

**Non-blocking**

1. `scripts/agent-test.mjs:135` still reads an agent-tree node `type` and the message does not cite it. `nodeNamed` only interpolates it into the error when one name hits several views. Which views match comes from the label and from `textOf` (`scripts/agent-test.mjs:118-121`): an inline image contributed `""` before and contributes nothing now, and those joins are the same (`scripts/agent-test.mjs:24-31`, already cited). `scripts/design-tool-canvas.test.mjs:103` asserts `canvas-open` is `Pressable`. `host/web-js/conform.mjs:192` is the comparison read of `n.type` (the message points at the field via `agent.js:125`); `host/web-js/conform.mjs:481` keeps only `Video`. Add those lines to the squash message if the re-grep list should name every read.

2. The message says the inventory was checked at `cb1b6c0101`. There the layout reply's copy is `host/web-js/agent.js:242` (`type: n.type`). Line 243 at that commit is the press-paint comment. After the comment added in `e9143679ed`, that copy is line 243. Parent lines 100 (`props.text`, only for `Text` / `SvgText` / `SvgTSpan`) and 125 (`type: type(el)`) are right. Cite 242 for that SHA.

3. `host/web-js/inspect-dev.js:270` copies `node.type` from `nodeOf`, which returns the inspect table's `entry.t` (`host/web-js/inspect-dev.js:235`), the plan site's type. It does not read `agent.js`'s classifier, so this report does not change when an inline image's agent type does.

The other cited lines match the files. `props.text` is asserted undefined at `host/web-js/inline-image-agent.test.mjs:41`. On the parent, `run(el)` is true for an `<img>` under a paragraph (`data-exact-text` is only on a non-run `Text`, `host/web/src/element.rs:875-879`; an image is emitted as `img`, `element.rs:565`), so the node is `Text` and `props.text` is `textContent` (`""`). Both assertions fail on the parent. `conform` folds a missing text and `""` together (`host/web-js/conform.mjs:192`). `host/web-js/document-agent.test.mjs` matches the parent. `scripts/async.mjs:113-116` leaves out only `bundle-size` and `document-agent`; the glob `host/web-js/**/*.test.{mjs,js}` includes `inline-image-agent.test.mjs`. The test was not executed.

APPROVE
