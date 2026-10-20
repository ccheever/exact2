# Code review: Tamber re-import judged whole (theme2's option B), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `0e495cd3e7`.
- **Method:** one brief (sha256 `619445fe687d9402c7b3b256943643483219c341254a0d894fda9d1ac23436d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; three should-fix (a settled token not re-read after a later pin or drop; dial followers settled against literals; the transaction's reason), nits (export order, test gaps)
- **Disposition:** taken in 33e803a09f: every member asked again each round, candidates alone, the reason says 'with the rest of the proposal', §10 says the export's order

Nothing in `0e495cd` is blocking. Refusals stay per token, the split is deterministic, both interaction cases are tested and written into §10, and the design that ships is still `check`ed. Two fidelity holes should be fixed before option B is trusted on reference-heavy or dial-heavy files: a token can be settled against a folding that is not the edit that ships.

## Findings

**1. should-fix — A token marked settled is never re-read after a later pin or a same-round drop.** `tamber/src/export_batch.rs:272-310`

`rounds` keeps every non-empty Kind and Colour member in the applied set, including ones already marked `done`, but `again` is skipped once `done[i]` is set. `Planned::Done` (`export.rs:1303-1333`), `Cleared` (ops cleared), and a Kind's single answer (`export_batch.rs:302-305`) all set that flag.

Two ways the folding under a settled token then changes:

- A sibling returns `Pins`. `more` stays true, the next round applies the new pins, and the settled token is not asked again. An unchanged side is still source text (`side_of` at `export.rs:1010-1014` keeps `side_text`, which may name another token). Its `Done` compared folded colours on the pre-pin folding. After the pin, that reference follows the new value, and the ops already chosen are what ship.
- In one pass, one token returns `Done` and another returns `Report` (the round-4 "no lines this design takes…" refusal, or a Kind whose `css` does not match). `more` is false, so `rounds` returns. The reported token's ops were in the folding the survivor matched, and they are not in `accepted` (`export.rs:1360-1363`).

`judged` on the way out checks that the shipped text is a valid design. It does not check that those colours still match the file. The pin loop exists for the `scheme=dark` case, so a palette token that gains a pin while another token still names it is a real import, not a constructed edge. Re-ask every non-refused member on any round that added a pin or dropped a member, and freeze `Done` only on a round that did neither.

**2. should-fix — Dial followers are settled as literal `SetToken`s, then shipped as `SetDial`.** `tamber/src/export.rs:1360-1389`, `tamber/src/export_dials.rs:236-244`

A colour token with `why: None` and non-empty ops is a member of `rounds`, so every other token's `Done` or `Pins` is computed against those literals. If it is also a dial candidate, the match after `rounds` sends it to `dials()` and not to `accepted`. When `SetDial` applies, `dials` pushes the dial op and holds the literals; they are emitted only from `fall`, when the dial cannot take the change.

The transaction then applies the dial ops first and the remaining token ops on that text. Tokens that pinned themselves to compensate for a follower literal, or that matched the file only because that literal was present, keep those answers. The follower's own line stays the dial reference, and the dial value moves. Settle the non-followers against the edit `dials()` actually returns (or run one more colour read of the transaction folding before accepting them).

**3. should-fix — The transaction reason still says the cause is the tokens before it.** `tamber/src/export.rs:1413`

Direct blame refuses the member whose line the diagnostic is on. That member can sit earlier in source order than the edit that made the value illegal (`a` naming `b`, with `b` later in the member list). Halving also refuses a later member only after committing the earlier half, so "before it" is sometimes true and sometimes false. The parenthetical diagnostic is the real reason, which keeps condition (1), but the fixed clause points at the wrong neighbours. §10 (`llp/1121-tamber.rfc.md:4231-4232`) describes that phrase as the old transaction report. The rounds path already says `the design would refuse it ({first})` (`export_batch.rs:261`). The transaction should use that sentence.

## Nits

**4. nit — §10's "source order" is the export's context order.** `llp/1121-tamber.rfc.md:4222`, `tamber/src/export.rs:816-823`

`names` walks `theirs.values().chain(ours.values())`. `ByContext` is a `BTreeMap`, and within a context the first source that declares a name wins. Halving, the refused sort, and the report all follow that vector, so the result is stable. It is not declaration order in the `.tamber` file. Say "export order" in §10.

**5. nit — The suite locks the ruling's refusal cases and the batch differential, and does not lock the fidelity holes or the table.** 

`export_batch::tests` covers both interaction cases and a multi-member report, and calls `judged` twice to lock determinism. The `g`/`h` case still refuses `g` on its own (`g` becomes `4px` while `f` reads it), so a halving where each half would succeed alone is untested. Nothing re-reads a `Done` token after a later pin, and nothing checks a sibling settled beside a dial literal that `dials()` then drops.

`edit/batch.rs`'s fuzzer (theme-fixture, Bluesky, 300 lists, fallbacks included) asserts `stepped_all` equals one-at-a-time. It does not isolate CRLF, two role spellings of one token, or a dial line above versus below a token. Those match `step` by construction (shared `Doc::apply` / `ended()`, `Index` first-of vs `token_index`, bottom-up targets).

`tamber/tests/it/scale.rs` is 2,000 tokens on one `dark-style` axis under `scheme=dark`, bounded at 10 s in debug. §10's release rows (400 → 0.08 s, 3,000 → 0.95 s, 2,000 × 64 contexts → 2.1 s, 3,000 refused → 0.7 s) and the "four densities report identically" sentence are not in this commit. The ruling's 800-token measurement is not in the table either (the alias-chain test is 8,000, a different path).

## Checked, and holding

**Batch vs `step`.** New `when` lines are inserted at `at.unwrap_or(len).min(len)`, and `same_conds` treats condition order as irrelevant, matching the parser's pair set. Indent, the value, and CRLF come from `Doc::apply` / `ended()` on both paths; the in-memory `When` only supplies line numbers for the next op on that token. An existing `when` with the same conditions skips the patch, and `Doc::apply` rewrites that line. Two spellings of one role resolve to one `Target` and stay in source order. `Index::new` is `or_insert`, the same first-of rule as `token_index`. A dial above or below a token is ordered by its line, bottom-up, so an insert cannot stale it. `valid` failures, parse failures, and any other op kind return `None`, and `stepped_all` steps those so the refusal text is the stepper's. A dial and a token cannot share a name in a design `check` accepts.

**Blame.** A member is refused for a diagnostic on its own token or dial line, including one another member's edit caused (the reader is refused, the cause lands). A diagnostic on a non-member blames the single member reachable through non-member reads; two members makes `blamed` return `None` and the whole set halves. One unattributable diagnostic does that for every diagnostic in the set. The walk terminates: each token index is queued once. Cost is one parse per `blamed` call plus a walk of identifier-like words, and only on the refusal path. A step `Err` (line 1, `tamber-edit`) skips `blamed` and halves. A line that is neither a member span nor a member dial (line 0, a gap, a dial line while the members are only `SetToken`) does the same. Refused indices are sorted, halves are index arithmetic, and member order is the `names` vector, so the report does not depend on hash iteration.

**Rounds, index, transaction.** Kind is one ask: the catch-all marks it done, and its ops stay in later rounds' applies. Empty ops are not members. `unchanged` compares the name line through the last `when` line, inclusive, each side in its own buffer. `folding.tokens[index]` lines up with `design.tokens[index]` on an `Ok` folding. The transaction's `k` counts only non-empty `accepted` entries, and `refused` is consumed in that order. Reports follow `names`, then dial refusals, then the transaction.

**Cost still in this path.** The per-token `check(current)` inside `reimport_ops` is gone; `before` is the design's folding. A successful re-import is at most four combined checks plus the transaction `judged`. `plan` still does its subset search once per unsettled token per round, `canonical` still checks a one-token design for a kind mismatch, and `marks` still scans with `token_index`. §10 is right to leave the 3,000 × 64 budget `LIMIT` to rt1. The DTCG `group` set is the dotted prefixes, read once, and matches the old `starts_with(name + ".")` test. `value()` borrowing and the per-plan `changed` set are sound.
