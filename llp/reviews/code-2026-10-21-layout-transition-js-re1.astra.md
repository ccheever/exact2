# Code review: The web JS target writes a state-dependent -exact-layout-transition (GitHub #809), narrow re-check 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `c71cb50dc2`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (the standing rule for a host change), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Clean; its note that index-driven gains share the item-driven deviation was folded into QUEUE.md after it.

Clean beyond the recorded deviation.

1. [QUEUE.md:3](QUEUE.md:3) accurately describes the missed pre-update snapshot. Its scope also includes **list-index-driven gains**, since indices update during `flush`. Existing curves and losses are handled; the limitation is capturing geometry when a stale region input newly enables a curve.

2. [rt.js:976](host/web-js/rt.js:976) fixes ghost-parent isolation. In [presence-glue.js:349](host/web/presence-glue.js:349), `create` updates tracking or stops motion without entering the restyled set. It does not create a DOM element or have another create-specific effect.

3. [rt.js:973](host/web-js/rt.js:973) is sound on the commit path: the probe reads committed `Now`, restores Listener/Owner, and removes its subscriptions, including after exceptions. No ownership leak or interference with other effects found.

4. Removing the Views deletion restores prior retention behavior. `Bound` still cleans up; [viewId](host/web-js/rt.js:747) remains unchanged.

Verified with six passing clock tests and read-only, in-memory probes of the relevant source. No browser run; no files changed.
