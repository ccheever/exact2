# Code review: LLP 1121 stage 3, commit 4, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `e486daba1711c1f69f3c26ce3bb1362446057bf40138b9b890d2e1be704a58cf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three medium; Grok: one high, one medium, one low.
- **Disposition:** applied without re-review

---

No high-severity findings. Three medium-severity findings remain:

1. **Medium — replaced design directories stay unwatched.** [scripts/tamber-server.mjs:158](/tmp/rv-s3c4/scripts/tamber-server.mjs:158)  
   With `tamber: "designs/app.tamber"`, removing and recreating `designs/` can permanently stall updates. The root watcher filters out the name `designs`, and `watchers.has(dir)` retains the watch attached to the old directory. **Fix:** observe relevant directory changes and invalidate/recreate watches when directories disappear or are replaced.

2. **Medium — timed-out writes lose their attribution when they later commit.** [scripts/tamber-server.mjs:135](/tmp/rv-s3c4/scripts/tamber-server.mjs:135)  
   After 120 seconds, the pending reply handler is deleted, but the queued transaction continues. Its eventual successful reply is discarded, as is its separate `changed` event. The next refresh announces the commit as a hand edit (`client: null`, `changed: null`). **Fix:** retain commit-completion processing independently of the HTTP timeout, publishing late successful writes with their original attribution.

3. **Medium — write events retain files already deleted before the snapshot.** [scripts/tamber-server.mjs:264](/tmp/rv-s3c4/scripts/tamber-server.mjs:264)  
   Delete `brand.tamber`, then commit an edit to `app.tamber` before the watcher settles. The transaction reply correctly omits `brand.tamber`, but both `publish` calls merge into `known` without removing it. Subscribers receive a stale file set until a later refresh. **Fix:** pass paths absent from the reply’s complete file set as `gone` to the first, unattributed publication.

In-memory probes reproduced these paths and confirmed first-hello recovery, `before` attribution, `after > seq`, disconnect handling, body destruction, and shutdown escalation. No dev loop or filesystem-writing tests ran; the worktree is unchanged.
