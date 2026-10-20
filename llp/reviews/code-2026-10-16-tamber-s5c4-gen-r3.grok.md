# Code review: `tamber gen`, the TS Token union, Rust constants (Tamber stage 5, commit 4), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `898124759bf316525049e688df0d1d58b1855a913a45ed2651d539c42957cfc1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** applied without re-review (the last round).

---

I'll review the stage 5 commit range read-only: round 2's fixes first, then whether anything from HEAD~2 through HEAD still blocks.Auto-compacting conversation (82% full)...
Conversation compacted.
Round 3 is a read-only check of the four round-2 fixes and whether anything in `HEAD~2..HEAD` still blocks. I'll verify those paths in the tree before writing findings.Round 2 is six files. I'll read the union, retention, and release-JS paths those fixes sit on, and check them against the whole `HEAD~2..HEAD` range.No findings. Nothing in `HEAD~2..HEAD` still blocks generated output, retention, the union, the CLI flags, or the release JS.

Round 2 holds:

- `contract types` passes the contract path into `typescript_exporting` with `design_exports` (`contract/cli/src/main.rs:234`). `contract_types_follows_exports` requires the union to be only `"badge.ink"` while the view still names `brand`.
- The web build passes `--app <appDir>` (`host/web-js/build.mjs:173`). `exact-web-js` drops that flag and its value, and for a baked plan reads `<app>/app.contract` for the union (`host/web-js/src/main.rs:98` and `:298`).
- `row_name` resolves a role override by the role spelling (`runner/src/runner/tamber.rs:125`). A non-tint override parses through the theme table to the design colour; `AccentColor` stays `ColorValue::Role` for the host.
- `admitted` refuses a camelCase name of exactly `__proto__` before writing `tamber.ts` (`contract/cli/src/tamber_gen.rs:266`).

Across the range, retention matches D7: role overrides, a computed colour, names the view writes, and for a bindable app the `exports` set or every token (`contract/lower/src/theme.rs:307`, `contract/lower/src/lib.rs:1139`). Theme-fixture, `tamber-modes`, and native-fixture do not gain theme rows from that clause, so release `app.js` stays as it was; declarations are the part that gain `ColorToken`. Generated names, tuple order, and the `ts`/`rust`/`-o` parser match the plan and the IR.

The suite was not executed. `~/x2review/stage5-brief.md` and `to-x2-perf4.md` were not readable here; the check used LLP 1121 D7/D12 and the commit messages.
