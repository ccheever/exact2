# Code review: the checker sweep (after the checker bound), round 3 of 3, 2026-10-24 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `03aba06dd3`.
- **Method:** one brief (sha256 `be0b41770b50ad20cb3251ec3e82e332a6495c0ee32ad812aaf8e1901a3594c6`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Round 3 of the checker sweep (`a50256a77d`..`03aba06dd3`, 34 files, +1544/−322). Round-2 blockers and the listed follow-ups hold. Two SHOULDs remain. Tests were traced, not re-run.

## BLOCKER

None.

## SHOULD

**`docs/tamber-protocol.md:219`** — tamber/1.17 still specifies the round-1 re-hold key.

The new ops-size paragraph says a re-hold may pass the 1024 cap when each op is “on a target (its kind and its token's or param's name)” the held proposal has. `target()` keeps every field except the values a re-hold may edit (`file`, `when` conditions, `before`, `kind`, `meta`, `comment`, axis identity). Round 2 updated the code comment; this protocol sentence is the same words and was added in this sweep.

The server is fail-closed (stricter than the text). A second implementation of 1.17 from the protocol would reintroduce the round-1 cap bypass. Spell the target as `target()` does.

**`tamber/src/edit.rs:521`** — a set-token/set-dial run that `batch::stepped` refuses still walks the run one `step` at a time.

`valid()` failures stop `take_while`, so the prefix is taken in one parse and the first bad op is stepped for its words (Astra’s empty-`value` case; `mixed_runs_and_long_axes_apply_in_moments` covers it). A value `valid()` accepts but the batch mini-parse does not (`x=y z`, the fuzzed fallback) makes `stepped` return `None` for the whole run. `run` then `step`s from the front: n full-design parses on the shared server, then the check refuses.

A paired write client can put that value on a re-hold of a server-planned import/re-import (values are exactly what `reholds` allows). Accept is not the isolate watchdog. Batch the prefix that would succeed, or refuse after the one failed `stepped`, and step only the op whose words you need.

## NIT

None.

## Equivalence of rewrites

| Rewrite | Same as before? | Evidence |
|---|---|---|
| **add-token batch** | Yes, including the when-line layout | `added_at` is the index after the last inserted line. First op still goes through `Doc::apply` (trailing comments, empty theme, CRLF via `ended()`). Later ops insert there. Tests: `a_new_tokens_when_line_like_the_last_one_stays_its_own`; `new_tokens_with_one_parse_land_where_stepping_puts_them` (empty theme, missing final newline, CRLF, kinds/meta/whens/comments) asserts `added == one_at_a_time`. |
| **Valid prefix + first bad op** | Yes for `valid()` / duplicate-name failures | `take_while` stops at the first `AddToken` that fails `valid()` or `names.insert`. Prefix in one parse; caller `step`s that op for its words. Empty last value is the 10s test. |
| **set-token/set-dial runs** | Yes on success | `the_batch_steps_as_one_at_a_time_does_over_fuzzed_operations` (300 cases × 2 designs). Mini-parse still refuses a line a later op would rewrite (`a_line_that_would_not_parse_is_refused_even_when_a_later_operation_rewrites_it`). |
| **Mixed add then set** | Yes on success | Consecutive runs: `added` then `stepped`. Each run matches stepping, so the concatenation does. `mixed_runs_and_long_axes_apply_in_moments` (1500 adds + 1500 sets on 20k tokens in 10s). Combined restores are one `Restore` per run; add-only and value-only tests assert `undo` returns the source. Mixed does not assert undo; `restore`’s prefix/suffix splice is still a true inverse of each run’s text. |
| **add-axis repeats** | Yes | Ordered `HashSet::insert` is the first duplicate `values[..i].contains` found. 100k values in 10s. |
| **scheme walk** | Yes | Aliases keyed as `same_name` (role id or raw name); BFS from `fixed` instead of a fixpoint scan. Repeated declared names skipped the same way. |
| **retention walk** | Yes | Every spelling still enters `words`; each lowercased bucket is expanded once (`expanded`). |
| **same_conds** | Yes on a when that names each axis once | Two-way `contains` at length ≤ 8; `HashSet` of pairs after that. History refuses a when that names an axis twice before it keys when-sets (`history.rs:170`). `landing.rs:29` still uses one-way `same`; that disagrees only on duplicate axes, which `valid` and check refuse. |
| **history** | Yes on versions `ambiguous` accepts | `ByName` first-wins (insert reverse); `branches`/`conds_key` are set identity for unique axes; `next_present` is one reverse pass. `same_order` on axis values is still a scan; QUEUE already has history/export membership as answered. |

## Security pass

Threat model holds on the doors that round 2 named.

- **1 MiB / 16 MiB.** `who.local` needs the development gate’s loopback (peer, host, no `forwarded` / `x-forwarded-*`). Remote, tunnel, and relay set `remote: true` and clear `local` before auth. A paired token is never `who.local`. `content-length` over the limit is refused before a byte is read; chunked bodies are counted as they arrive. After a local read past 1 MiB, only `method === 'import'` continues; unparseable bodies 413. Pairing test: 10 MiB DTCG import locally 200, same from a paired client 413, local 2 MiB `propose` 413.
- **Re-hold.** `target()` is the whole op minus editable values. Length cannot grow. Paired `client` is overwritten before the door. Propose-scope cannot replace someone else’s hold. `review.importer` does not skip `overOps`. Restoring/override is `local && review.importer` on a client propose; importHeld sets it itself. Tests cover file/axis swap, extra op, missing `from`, and a paired importer-marked hold.
- **Recursion.** Token walk and `$extends` at 64; `$ref`/set expand at 16; fold at `MAX_REFERENCE_DEPTH` (512); JSON `tooDeep` iterative at 64. Importer expansion is charged to `budget::IMPORT` / `REIMPORT`.
- **Echo.** Values and names over the css byte cap are refused by id and size, not by echoing the body.
- **DTCG / Figma re-import.** A theme group past `MAX_TUPLES` (64) is refused before name compares; `scheme(&names)` runs once a group. Figma `<Collection>.<Mode>` still calls `scheme` per mode; file count is 256, so that is small. Re-import is a DTCG export compare under `REIMPORT`, with the 4096-candidate when search already on QUEUE.
- **State vs disk.** The wire does not write design files. `accept` removes the proposal, then `write`/`ask`; failure restores unless conflict, then recompute. Digests guard apply. No new disagree path in this delta.
- **Remaining super-linear, already on QUEUE.** Token count uncapped; params laying, dial influence, export-batch refusal, re-import dials, narrowest-when. Export membership is still tokens × exports. History when-line lookup is still linear in that token’s `when`s. Grouping `$themes` by unique group name is still a scan of groups so far; at 1 MiB that is ~10⁴ groups, inside the 10s isolate watchdog, unlike round 2’s 20k-name × `scheme()` loop.

The round-2 add-token suffix match, add-axis pairwise scan, mixed-run per-op parse, 16 MiB-on-every-method, and DTCG pairwise-before-cap issues are closed.
