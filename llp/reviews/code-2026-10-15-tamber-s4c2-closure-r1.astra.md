# Code review: stage 4 commit 2: live or rebuild by the closure, round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `fbc5d771842e4c59707135c10ae08a6cc0e03112033b232200f7a269df83126a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both: failed builds cached as no synthetic; tokens that come to fold missed).
- **Disposition:** fixed in the first-round commit

---

I found two blocking correctness issues in HEAD (`0ac55c65`).

1. **[P1] Design-only recovery leaves the synthetic cache permanently incomplete** — [contract/cli/src/tamber_serve.rs:332](/tmp/rv-s4c2/contract/cli/src/tamber_serve.rs:332), [line 359](/tmp/rv-s4c2/contract/cli/src/tamber_serve.rs:359).  
   A failed compile is cached as an empty synthetic table, but design changes never invalidate it. Start with `brand = missing` and a row containing `color-mix(... brand ...)`; commit a fix to `brand`, then edit its colour again. The fix correctly requests a rebuild, whose plan now contains the synthetic. The following edit returns `live` with an empty `synthetic` batch, leaving the row’s folded colour stale. Failed extraction must remain distinguishable from an empty table, and design changes that affect retention must refresh it.

2. **[P1] Newly foldable synthetics are incorrectly classified as live** — [tamber/src/session.rs:215](/tmp/rv-s4c2/tamber/src/session.rs:215), [line 233](/tmp/rv-s4c2/tamber/src/session.rs:233).  
   `lost` detects only expressions that stop folding; failures on either side then remove the expression from both folds. Consider a mix stored in an action’s string literal, with `brand = AccentColor`: the compiler legitimately drops that loose synthetic. Changing `brand` to a literal colour makes the compiler retain it, but planning returns `live` without adding the synthetic. The running plan lacks that entry, and `set_token` cannot create it. A fresh server also misses this case because extraction examines only the pre-edit plan. Synthetic membership changes in either direction need rebuild classification.

The added tests cover neither recovery across committed edits nor gaining a synthetic. Role chains otherwise propagate effective value changes correctly; the vocabulary and `(` naming heuristic match the compiler. Other `.tamber` files are independent under the current compiler.

Cache hits still parse/resolve the source graph and reread/hash its files; I did not measure latency. Tests were not run: this checkout has no build artifacts, and the review was read-only. No files changed.
