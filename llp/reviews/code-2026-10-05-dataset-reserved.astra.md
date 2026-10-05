# Code review: the host's data names reserved (contract/lower/src/dataset.rs), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the reviewed commit.
- **Method:** one brief (sha256 `d0f3833ff0d30d425b76efb8cae6c4138b8f1b2860e9cf2420d2bdd07a8ba59c`), shared with grok. One round, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Taken: `agent-view`, `app-src` and `seq` reserved too (80 words); a census of every `*.contract` under ~/projects, ~/.tuft/projects and /private/tmp found no authored use of any of the six. One round, as asked; not reviewed again.

---

Two pre-existing omissions remain in [HOST_WORDS](/private/tmp/x2-peek-review/contract/lower/src/dataset.rs:22):

- **P2: `app-src`** — emitted for images at [rows.rs:560](/private/tmp/x2-peek-review/host/web-js/src/rows.rs:560) and written/read by [symbols.js:60](/private/tmp/x2-peek-review/host/web-js/symbols.js:60). Authored `data-app-src` can collide with the host’s image-source marker.
- **P2: `agent-view`** — accessibility inspection overwrites this attribute on app elements at [glue.js:1181](/private/tmp/x2-peek-review/host/web/glue.js:1181) and [agent.js:362](/private/tmp/x2-peek-review/host/web-js/agent.js:362), potentially replacing an authored value.

Also unreserved: `seq`, written at [dev.mjs:220](/private/tmp/x2-peek-review/host/web-js/dev.mjs:220). It appears only on a host-created checkpoint script; I found no app-element collision.

The commit’s three additions are correct: right list, exact spellings, alphabetical placement, and **77 unique entries**. The Rust property-writing path has no remaining unreserved names; its test does not cover the JS writes above.

Both requested test commands **should pass based on inspection**, but execution remains unverified: both stopped before compilation with `Operation not permitted` when Cargo tried creating its target directory in this read-only sandbox.

**Verdict: READY WITH CHANGES** — reserve `app-src` and `agent-view` to close the remaining app-element collisions.
