# Code review: refused kept answers journaled (c0f481711), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `7c9cdccf37e383cd0b59f35f059c75502eaeb5fa1eb456d24d79a01ad2d96ed2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both findings taken in the second commit: a standing commit journals its refusals, a restore lets them go; Grok's NIT taken.

---

**MAJOR** — A refusal that stands after boot is never written, and one a failed commit undoes stays recorded — `runner/src/runner/settlement.rs:450`

`refuse` appends to `keeping.refused` while settlement is still on its copy of resource state (`states`). That vec is drained only once, in `journal_kept_seeds` (`runner/src/runner/kept.rs:231`), which boot calls immediately after the first settle. A later commit before readiness clears the seed and calls `refuse` again — `an_identity_mismatch_discards_the_seed_and_changing_back_does_not_readmit_it` does this when `car` changes from `"A"` to `"B"` — and the line stays in the vec. `conclude` on a commit that stands (`runner/src/runner/commit.rs:118`) journals store writes and does not touch `keeping.refused`.

The same vec is outside the checkpoint. `refusal_before_readiness_restores_the_seed_and_its_identity_eligibility` visits `answer` with `car = "B"`, records `kept for ["A"], this launch asks for ["B"]`, then traps on `guard` before the pass publishes. The seed is restored (`kept_seed` still true, value still `"kept"`). The refusal entry is not. `refuse` then returns at `runner/src/runner/kept.rs:43` for that resource, so a later commit that really discards the seed keeps the undone reason and still never logs it. Boot itself is safe on this point: a settle that returns an error never reaches `journal_kept_seeds`, so a failed boot does not emit the line.

What holds in `c0f48171`:

- A refusal decided by the boot settle is journaled once, next to the “shows its kept answer” lines and before the `boot:` line. Decode failures are pushed only when an entry exists and `kept_seed` returns `None`, so they are not also reported from settlement. Admitted seeds, `data_ready` (the `!self.data.ready()` guard), and carried or checkpoint states (`kept_seed: false`, and `kept_seed()` is skipped when the slot is already filled) do not get a refusal line.
- The moved decode matches the old filter: an entry is used only when it decodes, the argument count equals the identifying arity, and `check_shape` accepts the value. A missing entry stays silent.
- Both argument lists go through `shortened` (200 characters, then `…`). The fallback and “no longer decodes or fits” lines do not print argument values. The three sentences match LLP 1027.005 D3.

The new tests cover those boot lines and that `data_ready` does not add another. They do not cover a commit that changes identity before readiness, or `act("refuse", …)`.

**NIT** — The test comment names an entry the test does not install — `runner/src/runner/kept_identity_tests.rs:281`

`a_refused_kept_answer_is_journaled_with_why` says a third entry no longer decodes, then launches two entries and asserts exactly two refusal lines. The undecodable entry is the next test.

The worktree also has uncommitted edits in `commit.rs`, `kept.rs`, and `kept_tests.rs`. They are not part of `c0f48171`, and this verdict is about that commit.

NOT READY
