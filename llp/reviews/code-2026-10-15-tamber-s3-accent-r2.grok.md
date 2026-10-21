# Code review: stage 3 commit 6 follow-up: role offers, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `c2ea48c92e864b755b0a42fd04a517874fde9b46255282e8e5e2e28a97b19518`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both P1: variation judged on the reconciled design; Astra P1: a bound role left in place).
- **Disposition:** fixed in the second-round commit; Astra's P2 not taken (safe direction)

---

I'll review the last two commits read-only, starting with the diff and the Contract rules those changes are supposed to follow.The last two commits are the shadcn accent offer and the follow-up that withholds axis-varying roles. I'll read those diffs against Contract's role-axis check.Contract's role-axis check is the standard `varies` has to match. I'll read that lowering path next to the importer, the CLI, and the wire.One blocking bug: a role can still be offered with operations and then fail to compile. `varies` looks only at the import’s mapped tokens, so a `when` that reconcile keeps on the design is invisible to it.

## Blocking

### `varies` misses a `when` the design already has — `tamber/src/import/shadcn.rs:357`

Contract refuses a role when the folded `css()` strings in `t.values` differ across tuples (`contract/lower/src/theme.rs:249`). That happens after reconcile, on the design that will actually be written.

`varies` (`tamber/src/import/shadcn.rs:406`) never sees that design. It returns true only when a mapped token has a `when`, or its default text names another mapped token that does. `reconcile` then keeps every `when` that names an axis other than `theme` (`tamber/src/import/mod.rs:159`). The offer is built after that check (`shadcn.rs:366`), with real operations.

`an_authored_when_on_another_axis_stays_and_a_role_offer_clears_its_own` is that case (`tamber/tests/import.rs:353`). The design has `shadcn.background` with `when density=compact: #fafafa`. The stylesheet is `:root { --background: #ffffff; }`, so the mapped token has no `when` and `varies` is false. The offer still has two operations (`import.rs:358`): set `-exact-background` to `shadcn.background` and clear the role’s own `when`. Accepting it leaves:

```text
shadcn.background = #ffffff
  when density=compact: #fafafa
-exact-background = shadcn.background
```

Those two tuples fold to different colours, so compile fails with `tamber-role-axis`. `accepted` only runs `exact_tamber::check` (`import.rs:27`), which does not apply that rule, so the test stays green.

The same hole covers the other cases you listed:

- A reference is the same miss. `shadcn.primary = shadcn.background` walks the mapped token, which has no `when`, while the design’s `background` still has the density line after reconcile.
- An axis `under scheme=dark` is the same miss. `varies` never reads `under_dark`. A kept `when dark-style=dim` still changes the per-tuple `css()` string. Contract already refuses that shape for `Canvas` (`contract/cli/tests/it/tamber.rs:554`).
- A `when` only on `theme` is handled. `theme` is owned, so reconcile unsets it and the offer is safe.

On an empty design the gate matches Contract. A mapped `when` becomes a note. `light-dark()` is one `Folded::Pair` per tuple (`tamber/src/fold.rs:145`), so a neutral import is offered with operations and compiles. Cyclic `var()` is inlined to the resolved colour before `Mapped` exists (`tamber/src/import/engine.rs:226`), and `seen` only breaks a leftover cycle. That cycle would be `tamber-cycle` on the token transaction itself.

Judge the role the way lowering does: fold the post-reconcile token for every tuple and compare `css()`. The witness test should expect a note and empty operations, and compile that design.

## Not blocking

**A `when` whose folded colour equals the default.** String-equal lines are already dropped (`tamber/src/import/engine.rs:322`), so those are offered normally. A different spelling that folds to the same colour (`#ffffff` versus `white`) keeps the `when`, and `varies` treats any non-empty `whens` as variation (`shadcn.rs:410`). The offer becomes a note even though every tuple’s `css()` matches and Contract would allow the role. Safe direction; the note overstates it.

**Re-import of a note.** Acceptable. The role is never written, so the note comes back on every call, including when the role was hand-set (`shadcn.rs:357` runs before the “already this token” skip at `370`). The wire prints it, then holds nothing when the token transaction is empty (`scripts/tamber-server.mjs:567` and `:570`). An unchanged theme-class source does not open a second review. The five console lines are repeated explanation. A source that does change correctly carries the note on the new review.

**CLI and wire shape.** An offer with no operations is represented correctly. The CLI JSON has `"ops": []` and the note (`contract/cli/src/tamber_import.rs:164`). The wire gives it `items: []`, keeps `note`, and does not shift later indices (`scripts/tamber-server.mjs:561`). Partial accept renumbers by `kept.indexOf` (`:425`); an empty list stays empty and the note is copied. `reviewShaped` allows an empty `items` array (`:299`).

**Tests.** `a_shadcn_import_with_every_offer_accepted_compiles` (`contract/cli/tests/it/tamber_cli.rs:126`) is a real compile of all three fixtures, every offer accepted, but only against `tamber app`. The theme-class fixture has no role operations, so this is what fails with `tamber-role-axis` on `-exact-background` if the gate is removed. It does not cover a design that already has the token. The theme-class unit test checks the five notes (`tamber/tests/import.rs:147`) and checks re-import operations only, which is consistent with the wire.

Nothing else in these two commits is blocking. Offering `--primary` as `AccentColor = shadcn.primary` matches lowering: the accent may be a colour or a token that folds to one.
