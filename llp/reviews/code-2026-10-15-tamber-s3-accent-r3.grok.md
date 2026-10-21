# Code review: stage 3 commit 6 follow-up: role offers, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `3e2f53a75895b5c834a70dfa50dff978769a55d9f0fd6c05eaf833853a008fe8`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Astra: removal breaks consumers, a role's own when lines, unpaired roles; Grok: other roles and spellings).
- **Disposition:** applied without re-review in the third-round commit: no removal op, every design role checked and named

---

I'll review the last three commits read-only, starting with the commit messages and the role-axis / importer code they change.The three commits are the shadcn role-offer change and two review rounds. I'll read `varies`, reconcile, and Contract's role-axis check against the accepted design.The when-line check is a sound over-approximation. Accepting every offer can still fail `tamber-role-axis`, because removal only covers the five paired roles and only under their declared spellings.

## Blocking

**Other roles that name the varying token are left in place.** `tamber/src/import/shadcn.rs:358`

Removal runs only for the role paired with that row (`-exact-background`, `-exact-label`, `-exact-secondary-background`, `-exact-separator`, `AccentColor`), and only when that row's own token `varies`. Any other role is untouched. `fill`, `Canvas`, and `secondary-label` are real roles.

```tamber
theme
  -exact-fill = shadcn.primary
```

with `:root { --primary: #ff0000 }` and `.theme-blue { --primary: #0000ff }` adds `when theme=blue` on `shadcn.primary`. `-exact-fill` still names it. Contract compares each tuple's `css()` (`contract/lower/src/theme.rs:240`) and reports `tamber-role-axis` at `theme.rs:254`. Accepting every offer does not remove `-exact-fill`. The same happens for an intermediate role: `AccentColor = label` and `label = shadcn.primary` removes `AccentColor` at most, and `label` still varies.

The direct case the tests use does work: `AccentColor = shadcn.primary` becomes `RemoveToken` of the design's own spelling (`shadcn.rs:378`).

**References are exact token names, not Contract's role spellings.** `tamber/src/import/shadcn.rs:439`

`varies` and `words` (`shadcn.rs:457`) match `t.name.value` only. Lowering resolves `label` and `-exact-label` to one token, and `Canvas` and `canvas` likewise (`theme.rs:38`, `check.rs:445`). A paired role written `AccentColor = label` against a token declared `-exact-label = shadcn.primary` is not treated as bound, so the offer is the empty "not offered" note (`shadcn.rs:373`) while the token operations still add the `when`. That is a false negative on the binding check. It does not affect the decision to add a new role override: importer values are colour literals or mapped token names, and those chains resolve by exact name.

## What holds

`varies` matches what `reconcile` leaves behind (`tamber/src/import/mod.rs:156`):

- A source `when` is kept or added, so a non-empty `whens` list counts as variation.
- A design `when` whose conditions are all on `theme` is unset when the source lacks it, and `varies` ignores it (`OWNED` at `shadcn.rs:422`).
- A `when` on any other axis is kept, including `theme` plus something else, and an unmapped token's `when` lines. Those count.
- Extra values on a theme axis the design already has do not by themselves vary a token. A `when theme=festive` the source does not have is owned, so it is unset before the role is set. A source theme `when` still counts.
- `light-dark()` is one `css()` string on every tuple (`fold.rs:142`). It trips the check only when a referenced token's slices differ, and that reference is walked.
- Dials are one default for every tuple, not an app axis. Profiles (`theme.rs:242`) only rewrite slices that already equal the default tuple, so they cannot create a new inequality.
- An axis `under scheme=dark` is not special-cased by name. Its `when` is kept when the axis is not `theme`, and `Pair` versus `One` really does differ in `css()`, so counting the `when` is required.

Colour spellings that fold to one `css()` (`red` and `#ff0000`) still count. That is the safe direction.

## Removal offer

For the binding it does see, the target is the design's spelling (`t.name.value` via `token_index`), and making it declinable is right: removal deletes an authored role. Declining leaves that role in place. The note at `shadcn.rs:365` says the token varies and names the role, and the build then fails with `tamber-role-axis` and the same rule (`theme.rs:257`).

## Tests

`a_role_whose_token_varies_once_accepted_is_not_offered` (`tamber/tests/import.rs:374`) and `a_shadcn_import_with_every_offer_accepted_compiles` (`contract/cli/tests/it/tamber_cli.rs:126`) lock the two cases from the last round: a kept `density` `when` is not offered, and `AccentColor = shadcn.primary` is removed, and both compile. The split authored-`when` test still clears the role's own `when` when the token is invariant (`import.rs:353`). Neither fixture has another role, or a second spelling, pointing at the varying token, which is why they stay green.

## Not blocking

- `scripts/tamber-server.mjs:567` prints `note` only when the offer has no operations, so the CLI line for a removal omits the reason. The note is still on the review object.
- `scripts/tamber-server.mjs:570` returns before proposing when `imported.ops` is empty, so a design that already matches the source never gets a removal-only offer. A declined removal is also not offered again on its own.
- `words` also treats function names as tokens (`light`, `dark`, `oklch`). A varying token with one of those names can prompt removal of a `light-dark(...)` role the fold does not read. Declining that offer still compiles.
