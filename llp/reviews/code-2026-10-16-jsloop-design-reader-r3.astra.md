# Code review: the JS loop's design reader (value-only edits without a build), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the whole range.
- **Method:** one brief (sha256 `c18a656b414f0ab7292b0af3c628a6c4c015ed9d529c948ab8098459a6573792`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (an overtaken check publishing dist; repair overlay; startup failure).
- **Disposition:** applied without re-review (the last round).

---

Blocking correctness issues remain:

- **P2 — Superseded verification can permanently leave stale code in `dist`.** [host/web-js/dev.mjs:238](/tmp/rv-s4-19/host/web-js/dev.mjs:238): `build()` already replaces `dist` before this guard. If a Rust module includes the design file, change a token and undo while verification runs. The superseded build publishes changed module code; the undo’s verification matches the original `logicRevision` and keeps that incorrect dist. Later pages run stale code with no reload scheduled. Reject superseded builds before publication.

- **P2 — A fast repair leaves the error overlay visible.** [host/web-js/dev.mjs:206](/tmp/rv-s4-19/host/web-js/dev.mjs:206): after an invalid design produces an overlay, repairing it clears `error` before `hadError` is captured. Successful verification therefore never sends `{ error: null }`. The page continues displaying the resolved error.

- **P2 — Failed startup leaves the detached reader build running.** [host/web-js/dev.mjs:131](/tmp/rv-s4-19/host/web-js/dev.mjs:131): Cargo starts before listening. If the port is occupied, line 562 exits directly without stopping its process group. A cold reader build continues consuming resources and holding Cargo locks after the loop exits.

Validated these paths using the actual functions with mocked I/O. Shape/accent/file fallback gates and scrub snapshot transitions passed those checks. No builds ran or files changed.
