# Code review: the binding core (Tamber stage 5, commit 3), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `d8523fe6013ce73f9ce060c0ec940e9d4faad9d3c017f44ef1fcdd51c6974255`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (inputs missed set_token and a place's scheme, rehashed the design; recorded names drained; refused values silent; view ids unchecked).
- **Disposition:** in the first-round commit.

---

Blocking: the observation marker can miss real changes.

- **P2 — Token edits can bypass observation.** [tamber.rs:95](/tmp/rv-s5c3-1/runner/src/runner/tamber.rs:95): `set_token("ink", "#ff0000", &[])` changes `color()` but updates neither `design` nor the role generation. `inputs()` stays unchanged, so observers skip the update. The test exercises only `apply_live`.

- **P2 — Scheme changes at the place are missed.** [tamber.rs:97](/tmp/rv-s5c3-1/runner/src/runner/tamber.rs:97): toggling an ancestor’s `color-scheme` from light to dark leaves every hashed input unchanged, although a `light-dark()` binding displays another colour. Only the system preference is hashed; `exact_scheme` changes are also absent.

- **P2 — Every observer rehashes the entire design.** [tamber.rs:95](/tmp/rv-s5c3-1/runner/src/runner/tamber.rs:95): before the first live batch—and throughout production—`design()` clones all tokens and slices, serializes them, and computes SHA-256 on every call. Polling after unrelated batches costs O(observers × design size). Cache the digest or use a change generation.

- **P2 — Invalid view IDs silently select another place.** [tamber.rs:17](/tmp/rv-s5c3-1/host/apple/src/tamber.rs:17): `4294967296 + existing_id` truncates to that existing node. Negative or string IDs silently select the root. Validate the type and `u32` range before resolving.

- **P3 — Colour reads disturb dependency recording.** [tamber.rs:79](/tmp/rv-s5c3-1/runner/src/runner/tamber.rs:79): parse token A, call `tamber.color(B)`, then `take_named()` returns zero: the binding drained A’s pending reads too. An enclosing `recorded` capture also acquires B. Isolate the lookup with the existing recording scope.

The “only then” test merely repeats a read without an intervening change; it does not establish stability across unrelated batches.

Static review; tests not run. Worktree and index unchanged.
