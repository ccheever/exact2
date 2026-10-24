# Code review: the design checker's bound (rt1's PRIORITY finding), round 3, narrow re-check 1, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `aa097f3aa0`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at aa097f3aa0 (before the rebase).

Clean. The two round-3 commits close each finding, and `reach.rs` and `profiles()` match the old results.

1. **Kind walk.** `measures` counts each function with `Walk::enter`, the same 512 `kind_of` uses for names. Past that it sets `deep` and `kinds` refuses with `too_deep` before another token is folded. A value is still at most 32 functions deep; the chain no longer multiplies the two.

2. **DTCG composites.** `inner` holds `bound::deeper()` across the composite and refuses with `too_deep`'s words. Alias hops stay on the existing 256 `visiting` cap. Along one path the stack is the sum of those caps, not the product.

3. **Role reachability.** `Index::get` is exact name, else the first spelling of the role, same as `token_index`. `names_reaching` is first-mapped-wins, follows the mapped value when there is one, and otherwise the design default. Reverse reachability matches the old `varies` boolean, including cycles. `split_roles` follows default and `when` words the way `chain` did, and the offers stay in design order. `orphaned_dial` is the same dial, from one set of the other tokens' words.

4. **Wire names.** `param_names` runs `over_name` before `resolve`. Resolve failures and unknown op names go through `bounded_words`.

5. **Profiles.** `pop_first` on the ready indices is the old `position()` scan: smallest index among direct `color(--…)` values and whole-name aliases of a colour already taken. `role_named`'s id, or the spelling itself, matches `same_name`. The carried colour is the first taken spelling's. Take order and the later whole-name refusal are unchanged.

6. **Icons.** The set is every token name plus each icon as it is declared, duplicates included, so the same name still fails `tamber-duplicate`.

7. **Astryx.** `source_bound` sees `local` strings. The following commit leaves source names to each importer; values still meet the shared bound.

8. **Kind on the wire.** `AddToken` uses `over_bytes("a kind")`, the same words as `preflight`.

9. **Whitespace.** The wire measures `v.trim()`, and a design line stores the value trimmed. `QUEUE.md` is right that dial influence is per tuple and tuples stop at 64 (`MAX_TUPLES`).

No new recursion, super-linear walk, echo, or wire/disk split in these commits. I reviewed the diffs and the old `chain`, `varies`, and `profiles` statically; I did not re-run the tests.
