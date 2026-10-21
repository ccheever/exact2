# Code review: refused kept answers journaled round 2 (a912b1bfa), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `15e28d00cc9c1c967d725ccc1ac3ef50fad660b7dbd2d2514d7e5254430d5207`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** No findings.

---

Every refusal before readiness is recorded once and journaled once.

- Boot drains the vector in `journal_kept_seeds` (`kept.rs:230`), immediately before the "shows its kept answer" lines and before the `boot:` line.
- A later standing commit drains it at `conclude`, before that commit's store-write lines and outcome line.
- An admitted seed never enters the block (`kept_seed` stays true). `data_ready` clears the mark only when `data.ready()` is set, and that path does not call `refuse` (`settlement.rs:444`). Carried and checkpoint states are installed with `kept_seed: false`, and the boot loop does not read a kept entry when a state is already present (`runner.rs:953`).
- After the mark is cleared it is not readmitted, so a second line is not recorded. `refuse` also ignores a repeat index while the vector still holds the first line (`kept.rs:43`).

The moved decode is the same predicate as before: an entry must exist, decode, have an identifying-argument count, and fit the shape. A missing entry still returns quietly. Only an entry that fails those checks is noted, as "it no longer decodes or fits", with no arguments (`kept.rs:255`). Both argument lists on a real refusal go through `shortened` (200 characters, then `…`), the same helper the shown line uses (`lines.rs:83`). The journal stays the development log the LLP already allows to carry URLs.

The LLP sentence in `llp/1027.005` matches the three lines. The tests cover mismatch, unknown identity, a bad entry, activation not adding a line, and the trap-then-real-change case.
