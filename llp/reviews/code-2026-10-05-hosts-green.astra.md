# Code review: exact-render and exact-linux green on main, 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox; rounds at `892ebc452`, `26119ca13`, `d9db489f8`.
- **Method:** one brief per round, the same sent to grok (sha256 `b5c9c97d5233facefea71f52025fe9a5d976232d92c64995278d2e5953ca8096`, `52d3d689c9545ea7039b11f9b29ddd8b650e717bbfed30a7d8b0b6355e119676`, `45a65593429b9199574b7b32cca7a0c28acb6b0997eb8d204be4566ec574f27d`); blind. Requested by the coordinator (Charlie: fix these if they are problems). The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** round 1 and 2 LAND WITH FIXES; round 3 LAND.
- **Disposition:**
  - Round 1, in `26119ca13`: (1) taken — a module is absent only when it is not there; an Android load or shader refusal on open is the reply's error. (2) argued — the unbound Linux text field's lost text predates the series (cbf1b3311) and needs a typed-text map read by edits, commit and paint; recorded in QUEUE.md, and the emoji test keeps testing the picker. (3) taken — 3f8cbb165 introduced the refusal.
  - Round 2, in `d9db489f8`: taken — absence is `Err(NO_IDENTITY)` or `try_exists() == Ok(false)` only, pinned by a unit test.

---

## Round 1

LAND WITH FIXES

1. **P2 — [host/linux/src/surfaces.rs:523](/tmp/rv-hosts/host/linux/src/surfaces.rs:523):** `Abi::open` also calls Android’s shader loader. Classifying every open error as nonfatal therefore swallows shader read/registration refusals there. Separate availability failures from shader failures, preserving named refusals, and cover that distinction. Desktop `prepare_shaders`/`commit_shaders` and readback checks remain intact.

2. **P2 — [host/linux/tests/pinned/host.rs:331](/tmp/rv-hosts/host/linux/tests/pinned/host.rs:331):** Adding `value=draft input=edit` masks a real uncontrolled-field bug. Enter correctly delays `change`, but `commit_text` reads the absent value prop and dispatches `""`. LLP 1069.001 D4 requires unbound controls to retain their value. Keep the uncontrolled case, retain typed text in the presenter, and verify Enter/blur payloads. This predates the series, but removing its coverage should block this test fix.

3. **P3 — [host/linux/tests/it/arrange.rs:412](/tmp/rv-hosts/host/linux/tests/it/arrange.rs:412):** The breaking-commit attribution is wrong: `425f97dab` changes documentation; `3f8cbb165` introduced `lower-reorder-collection`. Correct the attribution. The test adjustment itself is sound.

Commit 1’s guarded fallback is legitimate; its other changes and commit 4 are sound. No additional rules violations found. Builds/tests were not run, as requested.
## Round 2

LAND WITH FIXES

1. **P2 — [host/linux/src/surfaces.rs:527](/tmp/rv-hosts/host/linux/src/surfaces.rs:527):** `is_ok_and(|path| path.exists())` also treats permission/I/O errors and path-resolution failures (including missing Android `EXACT_NATIVE_LIBS`) as absence. Those failures become log-only, allowing successful agent replies with a broken configured module. Suppress only missing baked identity or confirmed file absence; propagate other errors to `Surfaces.error`. Add regression coverage for this distinction.
## Round 3

LAND

No remaining findings in 6c4444718..d9db489f8. Read-only review; no builds or tests run.