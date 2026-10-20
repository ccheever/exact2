# Code review: stage 4 commit 6: uses, resolve and scrub sessions, round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change commit and the first-round commit.
- **Method:** one brief (sha256 `3dcb3143ecdfc6f8ec244069a9ea328137f5633dcbd49dcee574135e1bc50c22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (a previous release's build overwriting the next scrub; an ended scrub announced only as opened; idle from preview end; `url(#…)` read as a token; slot holds from whole action bodies; comparisons beyond adjacent operands; `file` of another design).
- **Disposition:** applied in the second-round commit.

---

Blocking correctness issues remain:

1. **P2 — A previous release’s rebuild overwrites the next scrub.** [host/web-js/dev.mjs:137](/tmp/rv-s4-11/host/web-js/dev.mjs:137): release scrub A, then preview B before A’s rebuild finishes. That rebuild clears `scrubbed` and broadcasts A’s values. B remains open, but builds and `/__dev/design` show A until another update. Reproduced in memory.

2. **P2 — An ended scrub can emit only `opened`.** [scripts/tamber-server.mjs:502](/tmp/rv-s4-11/scripts/tamber-server.mjs:502): end the first preview while an update is queued. The queued reply contains `{ok: {scrub, ended}}`; this condition treats it as an opening. Since no preview had completed, no end event was emitted. Subscribers are left believing the cancelled scrub is active. Reproduced through the wire with a stand-in compiler.

3. **P2 — Idle still counts from preview completion.** [scripts/tamber-scrub.mjs:73](/tmp/rv-s4-11/scripts/tamber-scrub.mjs:73): `show()` resets the timer again. A preview taking 20 seconds therefore expires 50 seconds after the last update, not 30. Reproduced with controlled timers; the existing test’s immediate preview hides this.

4. **P2 — CSS references are mistaken for token reads.** [contract/cli/src/tamber_uses.rs:128](/tmp/rv-s4-11/contract/cli/src/tamber_uses.rs:128): with a token named `brand`, `fill="url(#brand)"` is reported as a direct use. It references an SVG element; changing the token does not affect that row. Filtering by colour codec does not make word matching sound.

5. **P2 — Slot dependencies use the entire action body.** [contract/cli/src/tamber_uses.rs:168](/tmp/rv-s4-11/contract/cli/src/tamber_uses.rs:168): an action assigning `label = "brand"` and `ink = "#ffffff"` makes `color=ink` a false use of `brand`. Conversely, copying a token-bearing slot into `ink` is missed because only literal strings are examined for assignments.

6. **P2 — Comparison exclusion handles only adjacent operands.** [contract/cli/src/tamber_uses.rs:101](/tmp/rv-s4-11/contract/cli/src/tamber_uses.rs:101): a colour conditional comparing `"brand" == (prefix + suffix)` reports `brand` as a colour dependency even when both result arms are literal hex colours. The comparison lies beyond the two-instruction lookahead. The comparison test uses `width`, which the codec filter excludes before this logic runs.

7. **P2 — `uses.file` can attribute another design’s sites.** [contract/cli/src/tamber_serve.rs:791](/tmp/rv-s4-11/contract/cli/src/tamber_serve.rs:791): requesting `brand` from an unused `other.tamber` still scans the app compiled with `app.tamber`. If both define `brand`, it returns the app’s sites although editing `other.tamber` affects none.

The ownership recheck, release-write exclusion, and cancellation during planning/first preview passed focused in-memory checks. Full build tests were not run; the worktree and index remain unchanged.
