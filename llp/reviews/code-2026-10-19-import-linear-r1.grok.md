# Code review: Tamber importers' super-linear paths, the exact part, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `c10631c116`.
- **Method:** one brief (sha256 `a6640f04ec2d4040839df9a7bb1860a55d89ee8a09a14566a95c7a8131894acc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** its note on stepped's claim is Astra's nit, taken

Nothing is blocking. No behavior change in the four rewrites.

The memo, the re-import transaction, the name maps, and `applied` all match the old results on the paths this commit actually runs. `export.rs` is at 1492 lines by the caps count (split on `\n`), `dtcg.rs` at 1471, both under the 1500-line cap.

**`type_of`.** Each token has one alias, so a walk is a single path and every node on it has that same answer. The new walk returns what the old one did: its own `$type` if it has one, otherwise the first typed node down `Some(Ok)` aliases, and `None` for a missing target, an alias `Err`, a non-alias, or a repeat. A cycle through the start is `None` (the old `seen` set rejected the edge back; `on` rejects the repeat before the second visit). A typed node stops the walk, so a later alias on that node is not followed, and that node is stored as `Some(its type)`, not `None`. Memo hits are taken before the cycle check, and only nodes visited in this walk are inserted, so a later start reuses the suffix instead of overwriting it. `type_of` holds the `RefCell` only across `alias`, `w`, and `defs`. It does not call `raw` or `leaf`. `leaf` calls `type_of` only after `raw` returns, so `raw` → `leaf` → `type_of` never runs inside an active `type_of`. No re-entrant `borrow_mut`.

**`stepped` + `accepts` versus `apply_all`.** Both refuse `Op::Restore` up front with the same diagnostic, then `step` with `params::NONE`, and check once at the end. `step` reads only the text before that op, so replaying the accepted prefix from the original text and stepping the next token on the cached text are the same string. The same step error or the same check `Vec` comes back, and the report still uses `.first()`. Dial ops in that loop have already passed `apply_all` inside `dials`, so the cached `Err` does not fire for them. If it did, every later non-empty token would report that same error and `ops` would stay the dial list, which is what a failing prefix step did before. `ops.extend` runs only when `trial` is `Ok`, so `made` and `ops` stay the accepted prefix. Empty groups still only increment `mapped`.

The comment on `stepped` overclaims in one case this caller does not hit: `apply_all` scans the whole slice for `Restore` before any `step`, while `stepped(a)` then `stepped(b)` returns `a`'s step error if `a` fails and `b` is the restore. Re-import only builds `SetDial` and `SetToken`.

**Name maps.** `last_by_name` overwrites in source order, so the last declaration wins, same as `rev().find`. `first_of` and both design maps use `or_insert`, so the first wins, same as `find`. `in_base` is presence either way. `enumerate().take(cur.tokens.len())` is the same index range as `tokens.iter().zip(&design.tokens)`, and the pair is still `(&tokens[i], &design.tokens[i])`. Exact name is tried before the ASCII-lowercase map, and the lowercase map is consulted only when `overrides` is set. `eq_ignore_ascii_case` and `to_ascii_lowercase` agree on ASCII case.

**`applied`.** `apply_all` already checked before it returned `Ok`, and `check` does not spend the import budget, so the old second `check` could not fail or change the folding. The `edited == current` branch still runs only on that `Ok`. In the colour loop it still clears and breaks before the token lookup. In `reimport_ops` the lookup still happens first, so a missing token is still "the token is not in the design any more" even when the text is unchanged.

**Tests.** Counts are semantic (`mapped > 256` on the 8000-chain, `2 * N` ops and no unmapped on 300 tokens). The bounds are 5s and 10s against the cited debug times of 0.25s and 1.2s, in line with the existing timing tests in this crate. They do not lock the op bytes; that is what the differential probe covered. The per-token whole-design check left for later is still there.
