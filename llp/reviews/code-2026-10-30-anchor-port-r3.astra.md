# Code review: the JS target's port of #1095's collection anchor, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `0744cf9b2b`.
- **Method:** one brief (sha256 `c1abee0a156484cd73d348d20fd0cdef0db69c7dd031e1f9a30e6a191e725832`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

All three round-2 items are resolved:

- [list.test.mjs:829](host/web-js/list.test.mjs:829) rejects non-finite restores; assertions require 2,000 checked, 2,000 finite, and zero misses.
- [list.test.mjs:789](host/web-js/list.test.mjs:789) supplies an off-end input, so an identity `anchorOffset` fails.
- [list.test.mjs:834](host/web-js/list.test.mjs:834) asserts `max >= 0`; the negative-max padded cases remain covered at [line 750](host/web-js/list.test.mjs:750).

The #1095 port is complete and matches `7b0352262f`’s changed semantics: capture fields, keyed-anchor defaults, every restoration branch, and port propagation through [ordinary capture](host/web-js/list.js:338), [report capture](host/web-js/list.js:687), and [reorder capture](host/web-js/reorder.js:91).

The tests are non-vacuous and would fail on the parent: [list.test.mjs:739](host/web-js/list.test.mjs:739) expects `269`, while the parent restores `250`. Further assertions cover growth, followed ends, rowless anchors, keyed defaults, and deleted rows.

Static review only; no tests or builds run. No additional defects found.

APPROVE
