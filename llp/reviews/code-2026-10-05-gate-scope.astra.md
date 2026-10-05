# Code review: what the gate does not test (AGENTS.md), 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, a detached worktree at `dd04cc87b`.
- **Method:** one brief (sha256 `53d08501820627eeafe9ae388a8e27bcbf2050efbdeba6e6d2438057aced5c85`), the same sent to both; one round, sized to a five-line documentation change; blind to the other review. Requested by the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition** (the landing commit):
  1. Taken: the bullet says the gate tests only `default-members`, and that a non-member is built only when a member depends on it.

---

LAND WITH FIXES

1. **P2 — [AGENTS.md:51](/tmp/rv-scope/AGENTS.md:51):** “never builds a member outside `default-members`” is false: default member `exact-bake` depends on excluded member `exact-gpu-reflect`. Compilation as a dependency does not run that dependency’s tests. Replace with: “The gate selects test targets only from `default-members`, excluding hosts such as …”

Otherwise sound: names, example command, post-push reporting, placement and length. Targeted existing tests preserve 794073089’s ~236→80-second tradeoff without widening `default-members` or adding forbidden apparatus.