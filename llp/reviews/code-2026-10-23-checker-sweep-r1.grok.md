# Code review: the checker sweep (after the checker bound), round 1 of 3, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `79dd820d6f`.
- **Method:** one brief (sha256 `3bf9ec8bf108f02b652dd9d48eafc4ae59768dd08d8807d409940a64516ec1ae`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Round 1 of the checker sweep (merge-base `a50256a77d`..`79dd820d6f`, 7 commits). One cap bypass, two CLI behavior changes, one wording nit. Findings are from reading the tree; the new tests were not re-run.

## BLOCKER

**`scripts/tamber-ops-limit.mjs:33` — a re-hold’s target is only `[op, name]`, so one held proposal over 1024 ops can be rewritten as edits that proposal never named.**

`reholds` allows a `propose` with `hold: true` and `review.from` to exceed 1024 when the new list is no longer than the held one and every op’s `JSON.stringify([op, op.name ?? null])` appears in the held list. Count and replace-rights hold (`:31` length, `:32` client or write scope). The target key does not.

Two ways that key admits a different edit:

1. **Ops with no `name` share one key.** `add-axis-value` / `remove-axis-value` are identified by `axis` and `value` (`docs/tamber-protocol.md:749-750`). `rename-token` is `from`/`to` (`:746`). `set-state`, `unset-state`, `set-row`, `add-param`, and `remove-param` are identified by style, state, row, component, and file (`:753-757`). All of one kind collapse to `["<op>", null]`. `every` then allows that key in every slot, so the whole list can be replaced with arbitrary ops of that kind. A themed re-import onto a design that already has the axis emits `AddAxisValue` beside the token ops (`tamber/src/import/mod.rs:142`). A revert of a hand-edited contract commit emits `set-state` / `set-row` with no `name` (`contract/cli/src/tamber_history.rs:559-560`). Both are server-planned, so both can sit past 1024 (`scripts/tamber-server.mjs:1117` and `:1172` call `hold` directly). The server already knows the real identity: `revertKey` uses style, axis, and value (`scripts/tamber-server.mjs:1468-1473`). Those kinds are outside `edit/batch.rs`, so each one is stepped.

2. **`file` selects the edited file and is not in the key.** `parse_ops` routes on `file` (`contract/cli/src/tamber_ops.rs:36`), and `hold` treats `op.file` as the touched path (`scripts/tamber-server.mjs:958`). A 3000-op `set-token` import, the case the ops-size test holds, can be re-held at the same length with the same names and a different `file` on every op.

Values are still checked afterwards. The door’s job is to refuse before any op is read, and this path does not. The key has to be the fields that identify the edit: `file`, and for each kind the fields `revertKey` and the protocol table already use.

Checked and not a bypass: the list cannot grow; a propose-scope client cannot re-hold someone else’s proposal (`allows` is read < propose < write); a paired client’s `client` is overwritten before the door (`scripts/tamber-server.mjs:553`); from the door to `hold`’s first line there is no await, so another request cannot drop or swap the proposal in between; `expire()` at the start of `hold` can drop an already-expired proposal, and the ops applied are still the ones just checked. A stray `ops` array on `accept` / `revert` / `import` is refused too (those handlers ignore it). `accept` items are indices into the stored list.

## SHOULD

**`scripts/tamber-import-review.mjs:68` — non-Astryx `tamber import` ignores `--app`.** The flag is still parsed (`:48`) and still documented (`:47`). The request is `{method:'import', kind, client, sources, prefix?}` and the server plans against its own app directory (`importHeld`, `scripts/tamber-server.mjs:1161`). Previously the CLI planned against `--app` and the hold failed the digest when that file was not the server’s. A CLI pointed at another directory now holds a proposal on the server’s design.

**`scripts/tamber-import-review.mjs:85` — Astryx still posts the full op list.** Other kinds ask the server to plan and hold, so they can exceed 1024. Astryx `propose`s with `hold: true` and the planned `ops`, so an Astryx theme over 1024 ops is now `tamber-ops-size`. Small Astryx imports still hold, and the nothing-to-propose line matches the old CLI. The wire still has no way to resolve Astryx, so this is the one importer the new cap actually refuses.

## NIT

**`tamber/src/bound.rs:130` — the `tamber-whens` message says “a token” while the loop at `:127` also covers params.** The id and the cap are right. A param over 64 when-lines is told it is a token.

## Checked, no defect found

The linear rewrites preserve the old result on the cases that can occur in a checked design. `ByName` inserts in reverse so the first declaration wins. `edits()` calls `ambiguous()` before `union`, so the new dedup never changes a successful union. `same_conds` is set equality up to 8 conditions and a sorted multiset after that; `valid()` already refuses a when that names an axis twice, which is the only place the old one-direction scan could disagree. Scheme aliases key by role id or raw name, matching `same_name`, and a repeated declared name is skipped the same way the old fixpoint skipped it. Theme retention groups by ASCII lowercase and walks each group in insertion order; the root set stays case-sensitive. The batch path returns `None` and falls back to stepping whenever the transaction is not only `set-token` / `set-dial`, and the single `Restore` is the prefix plus the suffix of the original lines.

No further super-linear pass showed up on the wire, an import, or a re-import inside this round’s scope. Tuple work stays under the existing 64-tuple cap plus these new caps. `budget::Work::begin` now wraps the tailwind, shadcn, and astryx imports, so spends inside them are charged. The DTCG tuple expansion is refused by `checked_mul` against the tuple cap before it runs, for a design that has already passed `check`.

The caps are inclusive (`axes.get(32)`, `whens.get(64)`), run in preflight before folding, and so `check`, build, and the server refuse with the same id on the wire and on the next check of a file on disk. Pencil refuses themes past its own 16 as `tamber-axes` before the rest of that import (`tamber/src/import/dtcg/pen.rs:780`). Other importers do not emit the id themselves; the preflight on the resulting design does. All three numbers are reachable: a design can sit on 32 axes and 64 when-lines, and the 33rd / 65th is the refusal. 1024 is a client-request cap. A theme import and a revert of a large contract commit are server-planned and can be larger, which is the exception the door is written for.
