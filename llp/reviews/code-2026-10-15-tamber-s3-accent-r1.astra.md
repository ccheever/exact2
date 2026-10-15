# Code review: stage 3 commit 6 follow-up: AccentColor offer and the wire tests in the async lane, round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `6fd2469ea1152e1d8606cb27529a21120ef23202e435cbaa06827d1c2b2fd042`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both: a role offer that varies by an app axis fails tamber-role-axis).
- **Disposition:** fixed in the first-round commit (varies, a compile-level test)

---

**One blocking issue in HEAD; no blocker found in HEAD~1.**

- **P1 — The accent offer can make the app fail compilation.** [shadcn.rs:349](/tmp/rv-ac1/tamber/src/import/shadcn.rs:349) offers `AccentColor = shadcn.primary` even when `primary` varies through `when theme=blue`, as in the blue-class fixture. The server accepts this, but Contract rejects axis-dependent role overrides with `tamber-role-axis` at [theme.rs:254](/tmp/rv-ac1/contract/lower/src/theme.rs:254). Accepting the tokens plus **only the accent offer** isolates this regression from the other four overrides. The [test helper:23](/tmp/rv-ac1/tamber/tests/import.rs:23) misses it because it invokes Tamber’s checker, not Contract compilation. Gate this offer on an axis-invariant resolved value and add compiler-level coverage.

- **P3 — Obsolete manual-edit metadata remains.** [import/mod.rs:35](/tmp/rv-ac1/tamber/src/import/mod.rs:35) still documents empty-operation offers and retains `Offer.note`, although every importer now supplies `None`. The CLI and wire still propagate it. This is nonblocking cleanup.

The remaining paths look sound:

- `AccentColor` is a system role; case-insensitive `token_index`/`role_key` resolution updates existing `accentcolor` declarations without duplication. An unchanged accepted import produces no offers.
- Ordinary colour-valued `shadcn.primary` satisfies `lower-theme-tint`; the blocker above is specifically app-axis variation.
- The CLI serializes the new operation normally. Item 27 and partial-selection renumbering work, including an existing override requiring multiple operations.
- Async registration includes the test exactly once. Bun 1.4.2 isolates parallel files; the temp fixtures and theme-fixture lock have no competing glue test. `deploy-paths.test.mjs` uses a containment assertion, so its expectation needs no update.

**Validation:** `caps`, scheduler assertions, and in-memory wire checks with a stub backend passed. Rust and dev-loop integration suites were not run under the read-only constraint. No files changed.
