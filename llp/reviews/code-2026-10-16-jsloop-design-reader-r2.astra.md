# Code review: the JS loop's design reader (value-only edits without a build), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the change and the first round.
- **Method:** one brief (sha256 `6200052f6c6b29100d87b1e685c8cc00c7c8fbaa35ee07d5098f322435da8474`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (cargo groups; the design as a module input; scrub freshness; stale reader; error overlay).
- **Disposition:** in the second-round commit.

---

**Blocking issues remain:**

- **P2 — Cargo descendants survive replacement/shutdown** — [dev.mjs:127](/tmp/rv-s4-16/host/web-js/dev.mjs:127), also line 518. Killing only Cargo’s PID leaves its `rustc`/linker children running on Unix. An edit during reader compilation can start a replacement while old children still write the same artifacts; stopping the loop can also leave them behind. Terminate and reap the build’s process group. [Cargo process model](https://doc.rust-lang.org/stable/nightly-rustc/cargo/util/job/index.html)

- **P2 — The app’s design can also change module logic** — [dev.mjs:187](/tmp/rv-s4-16/host/web-js/dev.mjs:187). If a Rust data module uses `include_str!("../../app.tamber")`, changing a token passes this gate but leaves the module in dist unchanged. Existing and newly loaded pages run stale logic. The slow path rebuilds that module and rejects `keep` through `logicRevision`.

- **P2 — Scrubs bypass the freshness fix** — [dev.mjs:149](/tmp/rv-s4-16/host/web-js/dev.mjs:149). After an unwatched `contract/` or `kernel/` edit, `scrubPreview → designOf` selects the old reader using only `tool.ready`. It can publish a preview the updated compiler would refuse. Check freshness for scrub reads too.

- **P2 — Edits during reader compilation can remain undetected** — [dev.mjs:143](/tmp/rv-s4-16/host/web-js/dev.mjs:143). If rustc reads a source, that source changes, then linking finishes, `fresh()` accepts the stale binary because its mtime is newer. Subsequent design edits keep using it. Validate against build-start inputs; this is why [Cargo rewinds its freshness timestamp](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html).

- **P3 — Error-overlay repair is incomplete** — [dev.mjs:192](/tmp/rv-s4-16/host/web-js/dev.mjs:192). Restoring an invalid design clears the server error but never sends `{error:null}`. Returning to the original values sends nothing at all, leaving the existing page’s error overlay visible.

The remaining stated fixes, batch digests, scrub snapshots, fallback gates, and shared lowering/`modes::every(true)` checked out. Three runtime tests and in-memory probes passed; a temporary-file test was sandbox-blocked. No worktree or index changes.
