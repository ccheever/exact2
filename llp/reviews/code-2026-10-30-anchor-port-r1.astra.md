# Code review: the JS target's port of #1095's collection anchor, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `192633e74d`.
- **Method:** one brief (sha256 `e996fe7d134293f3ef86d9b58ed16be4dd0fc2c9ebb4269381ee8c199fb7e46c`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

**Non-blocking**

1. [host/web-js/list.test.mjs:784](host/web-js/list.test.mjs:784): Ten animation frames do not guarantee collection readiness. Run alone, the test can reach `c.geometry` before the asynchronous controller import finishes ([list.js:944](host/web-js/list.js:944)). Await `exact.lists.pending()` and `exact.lists.settle()` before reading geometry. Also, the comment promises row growth, but both estimated and actual heights remain 50; exercise growth or narrow that comment.

2. [host/web-js/list.test.mjs:737](host/web-js/list.test.mjs:737): The principal Rust test is mirrored, but #1095’s sparse 25,000-row traversal assertion—`window.offset + beyond` at [index_tests.rs:878](runner/src/instance/collection/index_tests.rs:878)—has no JS mirror. Adding it would strengthen parity coverage. This is a test omission, not a missing runtime branch.

The runtime port matches #1095: `beyond`, raw offsets, padding, both clamping forms, followed ends, both rowless cases, surviving rows, and all-rows-gone restoration. All six `anchorOffset` calls pass the viewport, including inset and reorder. The principal mirror uses the same numbers and would fail on the parent at 250 versus 269; the unchanged all-rows-gone case appropriately passes there.

Existing coverage exercises chat following and inset changes through conformance, reorder through DOM tests, and saved positions through restoration tests. Cross-reference comments are accurate; the Rust diff is comment-only.

No tests, builds, network access, or edits performed. The required zero paging/startend failures, absence of new conformance failures, and caps result remain unverified by this review.
