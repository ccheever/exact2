# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), round 3 of 3 (security pass), 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7e3d0f7eb5`.
- **Method:** one brief (sha256 `b2a37ff0a45734ddb58408ab617becaeb54566b930a6993b076691b1d619a5a2`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

## BLOCKER

**`describe`, `resolve` and `explain` never hit the check cache, so a client can make the server pay overlapping-dial influence on every read** — `contract/cli/src/tamber_params.rs:183`

`check_design_cached` is keyed by the whole `Design`, `Vocab` and trace flag (`tamber/src/checked.rs`). QUEUE.md and `daae95f4cc` treat that as the per-request bound: a design asked about again, unchanged, is answered from the last four.

`Server::describe`, `resolve` and `explain` all go through `design_with` → `fold_with`, and `fold_with` still calls `check_design` (uncached):

```183:183:contract/cli/src/tamber_params.rs
    let folding = exact_tamber::check_design(laid(&design, app), vocab)?;
```

The cache is only on `components` (a second fold of the same laid design after describe has already paid), `Server::design` (export, unlaid), and a scrub `varies` helper. The integration test (`tamber/tests/it/limits_counts.rs` `a_design_checked_again_unchanged_is_answered_from_the_last`) calls `check_design_cached` directly, never the serve path.

A paired client looping `describe`/`resolve` on a disk design with overlapping closures pays that cost every time. The lock is fail-open (`lock().ok()` recomputes) and the key is complete (design + vocab + `trace::on()`); the hot reads never look it up. `explain` is the same miss inside `traced`.

Wire `fold_with` to `check_design_cached`. Keep writes, re-import, probes and `plan` on `check_design`. Drive the cache test through `fold_with` or a serve `describe`.

## SHOULD

**1. After a fallback settlement spends the budget, `dials` still `apply_all`+`check`s later dial groups** — `tamber/src/export_dials.rs:255`

`reimport_within` checks `spent` before calling `dials` and after it returns, so the `Import` is dropped and `marks` does not run. Inside `dials`, a `literal()` that calls `settle_alone` can set `OVER` on an early group; later groups still take the uncharged `check` at the `apply_all` match. Dials grow only on disk, so this is leftover work after the budget. `take` / `settle` already return when `spent()` is set; this loop does not.

**2. Token-naming ranged values still cost one full `check_design` each** — `contract/lower/src/params.rs:603`

Independence now folds a line alone if any value names a param or a design token, which restores the round-2 range check (`a_value_reading_a_param_through_a_token_is_checked_against_the_design`). Literals and dial-reading values batch (the 1,500-param tests). The carve-out is params × check on that shape. `add-param` is a tamber/1 Contract op under the same 1 MiB / 1024-ops door, so param count can grow, and every later design write re-runs `written_ranges` over the whole params block. The 1,500-param timing test only sets literals.

## QUEUE (accepted remainder)

Dial influence on overlapping closures: token *k* reads dial *k* and names token *k−1*; about *n²/2* folds; **500 dials/tokens 1.5 s, 1,000 6.7 s, 2,000 50 s** in a release build, times tuples (≤ 64). Dials grow only on disk (`parse_ops` has no add-dial; `restore` is rejected; `no_client_operation_adds_a_dial` covers the design ops). The per-request trigger is an unchanged `describe`/`resolve` of such a design, which is the BLOCKER above.

## Round-2 follow-ups

| Claim | Status |
|---|---|
| `written_ranges` folds alone any value naming a param or design token | Fixed (tested) |
| Package params’ app lines by one Resolver | Fixed |
| Re-import decides each dial before any literal | Fixed (1,000-token / 3,000-fallback tests) |
| Dial-reading values batch with a dial reader beside them | Fixed (tested) |
| `shown()` by lookup at each token start | Fixed (equality-tested against the old replace) |
| `folded()` over a name map; levels by name once | Fixed |
| Empty blame round charges nothing | Fixed |
| Nothing more asked or marked once the budget is past | Incomplete (`dials` leftover, SHOULD 1) |
| Server reads answer from the last four checked | Incomplete (BLOCKER) |

Cache key inputs confirmed: `Design` (content `PartialEq`, including line/col), `Vocab`, `trace::on()`. `check_uncached` mutates a clone (`icon::aliased`); the cache stores the unmutated key and a cloned `Answer`. Poison fail-opens. Writes and re-import stay on `check_design`. Ordinary shared-dial re-import still fits `REIMPORT`. Resolver first-wins, `shown` equality, and `folded`/`levels` first-wins match the old answers; valid plans stay byte-identical.

No NIT.
