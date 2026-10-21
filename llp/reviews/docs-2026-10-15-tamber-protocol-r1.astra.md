# Docs review: docs/tamber-protocol.md (the Tamber protocol's reference), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `80a292c52`.
- **Method:** one brief (sha256 `50452d554d32302ff0c7584730032af302e5d2e72a259a74160bec7edb7ceb3c`), shared by both reviewers, blind to the other review. The author (Claude, theme2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 blocker, 16 should-fix, 3 nits).
- **Disposition:** the page rewritten against the code. Taken: the error and status table (bare 404s, 421 from the gate, 400/409/500/504 cases); a 504 does not cancel a write; no client-version negotiation; the 1 MiB less 64 body limit; extra design files only in the app directory; edits splice touched lines (not whole-file fmt); the IR has no `when`s; the shapes of `resolve` and `explain`; `changed` includes dependents; `undo` returns a new inverse and ids are single-use; `accept` with no items applies role offers; a held propose ignores `requires` (files not edited become guards; queued as a wire fix); a recomputed proposal is not written; `history` compares parsed declarations, first-parent; revert refusals by edit (an added axis, not a removed one); import shapes and defaults; subscribe replays nothing, `epoch` is checked only when sent; the journal kept when it cannot be judged. Not taken: the `tamber tool` command and the design-tool README, which landed on main (`9bdf8cf62`) after the reviewed commit; LLP 1122 §10 now exists (same push).

---

Not fully accurate. I treated the listed pending 1.1 fields as intended. Review stayed read-only.

1. **Blocker — wrong:** [docs/tamber-protocol.md:147](/tmp/rv-protodoc/docs/tamber-protocol.md:147): held `propose` requests silently discard `requires`. Only extra entries in `files` become held guards, so acceptance can write without checking an advertised dependency. See [tamber-server.mjs:373](/tmp/rv-protodoc/scripts/tamber-server.mjs:373).

2. **Should-fix — omission:** [docs/tamber-protocol.md:244](/tmp/rv-protodoc/docs/tamber-protocol.md:244): omitting `accept.items` accepts **every operation, including role offers**. Keeping offers off requires explicit indices, followed by acceptance of the replacement proposal. See [tamber-server.mjs:509](/tmp/rv-protodoc/scripts/tamber-server.mjs:509).

3. **Should-fix — omission:** [docs/tamber-protocol.md:53](/tmp/rv-protodoc/docs/tamber-protocol.md:53): a 504 does **not cancel the write**. It can subsequently commit and emit events; clients must reconcile before retrying. See [tamber-server.mjs:173](/tmp/rv-protodoc/scripts/tamber-server.mjs:173).

4. **Should-fix — wrong shape:** [docs/tamber-protocol.md:75](/tmp/rv-protodoc/docs/tamber-protocol.md:75): `files` is not universally a digest map. `commit` replies and `committed` events contain `files: [path, …]`. Specify that exception at line 195. See [tamber-commit.mjs:200](/tmp/rv-protodoc/scripts/tamber-commit.mjs:200).

5. **Should-fix — wrong shape:** [docs/tamber-protocol.md:261](/tmp/rv-protodoc/docs/tamber-protocol.md:261): proposal events and open-list entries omit `files`; they are not the complete held-`propose` reply. See [tamber-server.mjs:456](/tmp/rv-protodoc/scripts/tamber-server.mjs:456).

6. **Should-fix — wrong:** [docs/tamber-protocol.md:92](/tmp/rv-protodoc/docs/tamber-protocol.md:92): `describe.ir` does not contain authored `when` clauses. It contains folded `values` and tuple-index `entries`; clients obtain clauses through `explain`. See [ir.rs:140](/tmp/rv-protodoc/tamber/src/ir.rs:140).

7. **Should-fix — wrong:** [docs/tamber-protocol.md:220](/tmp/rv-protodoc/docs/tamber-protocol.md:220): history compares **parsed declarations**, not checked IR. Checker-invalid versions can yield edits. The size cutoff applies to the serialized comparison request containing both versions, not each version separately. See [tamber_history.rs:52](/tmp/rv-protodoc/contract/cli/src/tamber_history.rs:52) and [tamber-server.mjs:547](/tmp/rv-protodoc/scripts/tamber-server.mjs:547).

8. **Should-fix — wrong:** [docs/tamber-protocol.md:231](/tmp/rv-protodoc/docs/tamber-protocol.md:231): a removed axis **can** be restored with `add-axis`; an added axis cannot be removed. Changes to an axis’s `under`, the design name, or exports are also unsupported. See [history.rs:247](/tmp/rv-protodoc/tamber/src/history.rs:247) and [history.rs:292](/tmp/rv-protodoc/tamber/src/history.rs:292).

9. **Should-fix — wrong:** [docs/tamber-protocol.md:14](/tmp/rv-protodoc/docs/tamber-protocol.md:14): there is no implemented major-version negotiation/refusal. `hello` reports a constant protocol version without checking a requested version; clients must validate it themselves. See [tamber_serve.rs:553](/tmp/rv-protodoc/contract/cli/src/tamber_serve.rs:553).

10. **Should-fix — wrong limit/status:** [docs/tamber-protocol.md:46](/tmp/rv-protodoc/docs/tamber-protocol.md:46): the HTTP body cap is **1 MiB minus 64 bytes**. Oversize handling destroys the request/socket before attempting 413, so a client can receive a connection failure instead. See [tamber-server.mjs:72](/tmp/rv-protodoc/scripts/tamber-server.mjs:72) and [tamber-server.mjs:248](/tmp/rv-protodoc/scripts/tamber-server.mjs:248).

11. **Should-fix — incomplete errors:** [docs/tamber-protocol.md:47](/tmp/rv-protodoc/docs/tamber-protocol.md:47): document bare `{message}` authentication 404s, empty-body 421 gate rejections, and internal 500s. Browser POSTs with an Origin host/port different from Host fail the gate even with a token. See [tamber-server.mjs:85](/tmp/rv-protodoc/scripts/tamber-server.mjs:85), [tamber-server.mjs:400](/tmp/rv-protodoc/scripts/tamber-server.mjs:400), and [install-page.mjs:288](/tmp/rv-protodoc/scripts/install-page.mjs:288).

12. **Should-fix — wrong scope:** [docs/tamber-protocol.md:73](/tmp/rv-protodoc/docs/tamber-protocol.md:73): additional `.tamber` files come from the **app root**, not beside a nested manifest-selected design. Paths are app-relative. See [tamber_serve.rs:466](/tmp/rv-protodoc/contract/cli/src/tamber_serve.rs:466).

13. **Should-fix — wrong:** [docs/tamber-protocol.md:132](/tmp/rv-protodoc/docs/tamber-protocol.md:132): writes do not run whole-file `tamber fmt`. They splice touched lines in formatter layout and preserve other bytes; undo restores original bytes. See [edit.rs:1](/tmp/rv-protodoc/tamber/src/edit.rs:1).

14. **Should-fix — omission:** [docs/tamber-protocol.md:174](/tmp/rv-protodoc/docs/tamber-protocol.md:174): successful undo consumes its inverse ID and returns the normal write reply with a **new inverse for redo**. Reusing the consumed ID returns 400. See [tamber_serve.rs:757](/tmp/rv-protodoc/contract/cli/src/tamber_serve.rs:757).

15. **Should-fix — unavailable command:** [docs/tamber-protocol.md:284](/tmp/rv-protodoc/docs/tamber-protocol.md:284): HEAD has no `tamber tool` launcher, port-7740 proxy, or referenced `apps/design-tool/README.md`. The command falls through to the compiler and fails usage validation. Mark this separately as forthcoming. See [exact.mjs:896](/tmp/rv-protodoc/scripts/exact.mjs:896).

16. **Should-fix — omitted limits:** [docs/tamber-protocol.md:123](/tmp/rv-protodoc/docs/tamber-protocol.md:123): token `kind` currently supports only `color`, and axes may produce at most 64 tuples. These materially constrain advertised operations. See [check.rs:481](/tmp/rv-protodoc/tamber/src/check.rs:481) and [check.rs:305](/tmp/rv-protodoc/tamber/src/check.rs:305).

17. **Should-fix — omitted defaults:** [docs/tamber-protocol.md:238](/tmp/rv-protodoc/docs/tamber-protocol.md:238): specify that `source` is inline text, `sources` accepts 1–256 files, and default prefixes are `shadcn`, `tw`, and `dtcg`. Also, `review.tokens` counts non-offer operations, including axis operations. See [tamber_import.rs:115](/tmp/rv-protodoc/contract/cli/src/tamber_import.rs:115) and [tamber_import.rs:143](/tmp/rv-protodoc/contract/cli/src/tamber_import.rs:143).

18. **Nit — unclear:** [docs/tamber-protocol.md:142](/tmp/rv-protodoc/docs/tamber-protocol.md:142): `class: "live"` is currently advisory classification, not actual live application. The dev loop still rebuilds; clarify this alongside the later-stage note. See [dev.mjs:128](/tmp/rv-protodoc/host/web-js/dev.mjs:128).

19. **Nit — overstatement:** [docs/tamber-protocol.md:54](/tmp/rv-protodoc/docs/tamber-protocol.md:54): optional fields commonly treat explicit `null` as absent, including `file`, `files`, `requires`, and `when`. See [tamber_serve.rs:1017](/tmp/rv-protodoc/contract/cli/src/tamber_serve.rs:1017).

20. **Nit — wrong:** [docs/tamber-protocol.md:283](/tmp/rv-protodoc/docs/tamber-protocol.md:283): tool serving permits symlinks resolving inside the build to non-dot paths. It prevents escape through symlinks, not all symlink traversal. See [tamber-server.mjs:279](/tmp/rv-protodoc/scripts/tamber-server.mjs:279).
