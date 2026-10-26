# Code review: the design checker's bound (rt1's PRIORITY finding), round 3, narrow re-check 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `cfbb6dd5fa`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at cfbb6dd5fa (before the rebase); the next commit changes only that test's fixture and comment.

**Clean.** The re-check 1 blocker is closed, and the names that reach a holding definition are the same.

`names_reaching` still has one name node per distinct spelling, but that node stores a single edge to a definition node. The definition is keyed by the pair `(mapped index of that exact name, design index Index::get resolves it to)` and is built once (`tamber/src/import/reach.rs:119-152`). Only the definition walks its value. Distinct pairs are at most one per import token plus one per design token, so the edges are the spellings plus each resolved value's words, however the spellings are cased.

The value choice is unchanged: the first mapped token for that exact name wins (`reach.rs:90-91`), and a mapped value is followed before the design default (`reach.rs:140-144`). `holds` runs once on that same pair (`reach.rs:137-139`), so every name that used to be a seed still reaches the one seed for its pair, and the word edges out of that pair are the same. A cycle stays in the existing iterative reverse walk (`reach.rs:51-72`, `reach.rs:121-154`); nothing new recurses, and this path builds no diagnostic text. `role_spellings_are_read_once_each_in_role_analysis` does drive that path: the quoted families are split into role spellings, and those spellings share the one `-exact-secondary-background` definition.
