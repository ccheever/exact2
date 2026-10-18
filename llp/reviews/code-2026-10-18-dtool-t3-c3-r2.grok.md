# Code review: LLP 1122 T3 step 4, the re-scope, round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, the same throwaway worktree at `5c7138e94` (`2d59d8578` after the rebase).
- **Method:** one brief (sha256 `aee04452d09a6031cad46b26b2e25916860fca7db40315d1b56f0ff2cd68e94d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high; smaller points).
- **Disposition:** taken in `4b3664868` and `afb9bf7f4`: the other rung's value is the one the moved line's copies show (its most specific when that the edit's when holds, else its unconditional value), so a finer mode moves to the alias it shows; an icon param is skipped; the app's design opened by its file offers the move; any server refusal of the move withdraws it (a loop through another token included); the move requires every file it was worked out on, so a component default changed in its Contract source since the edit refuses it (the real-server test drives that). Not taken, as limits for §10: a param's first when line (two operations) offers no move; a loop through another token is refused by the server rather than foreseen.

The exact-`when` case from round 1 is fixed. A finer mode still re-scopes to the unconditional alias, and that write succeeds.

## Blocks

**[P1] A mode covered by a shorter `when` is treated as the unconditional alias — `apps/design-tool/rescope.ts:48`**

`branchOf` keeps a line only when its conditions are exactly the edit's `when`. Otherwise it uses the unconditional value (`w?.value ?? x.value`). The checker, and the branch editor in `app.ts`, use the most specific `when` whose conditions all hold. Those differ as soon as the design has two axes (the stub does).

```
ink = #010101
  when dark-style=dim density=compact: #222222
paper = #ffffff

params
  Card.fill = ink
    when dark-style=dim: paper
```

On Dim+compact, `ink`'s own branch is `dark-style=dim density=compact`. Change it to `#abcdef` and Apply. `Card.fill` has no line with that exact `when`, so the scan falls through to `ink` and offers "Apply to Card.fill instead". On that copy `Card.fill` is showing `paper` (`dark-style=dim` wins). The transaction restores `ink` and writes `Card.fill`'s Dim+compact line to `#abcdef`, which beats the dim line. The copy you edited goes back to `#222222`. The param, which was white, becomes `#abcdef`.

The same function picks the token for a param edit (`rescope.ts:64`). On that Dim+compact copy, open `Card.fill` (it shows `paper`), choose Only this mode, and set `#abcdef`. The offer says "Apply to the token ink instead", and the write lands on `ink`. The alias on that copy is `paper`.

Round 1's example, editing the dim line itself, is fixed: that `when` matches exactly and the tests cover it. They never use a mode the winning `when` covers but does not equal.

## Earlier points

- Intervening write: the move is sent with the digests the edit left (`app.ts:884-895`), not a fresh propose. A conflict drops the offer (`app.ts:899`). The real-server test drives the refusal.
- One revision: the design read and every `explain` must be that proposal's digests (`app.ts:702-710`, `rescope.ts:56`), or there is no offer.
- Redo stores no move (`app.ts:1028`).
- Role spellings go through `tokenOfIn` (`rescope.ts:29-33`), in line with the checker's `role_key`.
- Scrub Keep computes a move from the files the scrub opened on (`app.ts:951-962`).
- An expression still has no re-scope. Its inputs stay on the branch editor, as the round-1 note kept.

## Smaller

- The app design opened by its path never offers a move. The file menu sets `file` to that path, and the panel edits there, but `rescopeFor` returns null whenever `file` is set (`app.ts:705`) and only looks at reads stored under `""`.
- A param with no line yet, given its first `when`, is two operations (the default line, then the `when`). `writeEdit` attaches a move only when `ops.length === 1` (`app.ts:873`), so that edit has no offer. It is the case where the alias is the default and the `when` should move onto the token.
- One param with `when` lines that `explain` cannot read aborts every token move. An icon param is such a param (`tamber_serve.rs` answers that `explain` takes a token). The scan treats that failure as "no offer" for the whole edit (`rescope.ts:79`), including params that do alias the token.
- A value that only reaches the destination token through another token is still offered. The server rejects the cycle and leaves the files alone, but the button stays: only a conflict clears it (`app.ts:899`).
