# Code review: the date and place at launch on Apple, the wasm host and Linux, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `561be9a344063a14163ad947620ef22e5bb8be32be1efd0a32654bc560b3b5be`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's two MAJORs (the locale after the initializers; a renderer-clock checkpoint's date) and Grok's (Linux's two date samples) taken in round 2's code.

---

- **MAJOR — [host/web/glue.js:1305](/Users/admin/x2p2/apps/exact2-comp/host/web/glue.js:1305):** The launch epoch uses the page’s elapsed clock, but non-agent checkpoint boots retain the renderer’s Unix-millisecond clock. Consequently, `epochAtZero + performanceNow()` approximately doubles the timestamp during the first settle, invalidating date-dependent checkpoint answers. Compute launch facts against the candidate runner’s clock.

- **MAJOR — [runner/src/runner.rs:898](/Users/admin/x2p2/apps/exact2-comp/runner/src/runner.rs:898):** The locale override happens after `init_slots` has evaluated root state initializers. With a French launch locale, `state label = t("title")` still captures the base translation permanently; the later identical `set_place` cannot repair it. Resolve the launch locale before evaluating ordinary slot initializers.

Focused prebuilt tests, in-memory glue checks, caps, and boot checks passed. A broader filter also selected a file-store test that failed under the read-only sandbox. No files changed.

NOT READY
