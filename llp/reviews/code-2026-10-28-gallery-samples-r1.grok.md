# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `d9ffe61393`.
- **Method:** one brief (sha256 `8109d7026c4a95eebd34e29f608f962a502c09bb2f5404d3e46aa9df98af9ee5`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the gallery colour-role and cell-alignment fix as a read-only pass, starting with the commit and the files you named.This is a focused read-only review of commit `d9ffe61393`, not a branch-wide review. I'll inspect the diff and the colour-role and alignment paths it changes.The diff is small. Next I’ll trace how cells are laid out and how colour samples are chosen, including child components and hosts that reject a role.SHOULD — Every cell column stops stretching its component, so full-width rows shrink to content width. `contract/cli/src/gallery.rs:942`

`align-items="flex-start"` is on the cell column, and a column's default cross alignment is stretch. FollowButton's pill is `width="auto"`, so this keeps that pill at its own width. PostCard, PersonRow, ActionRow, ProfileHeader, AtCard, SwitchRow, ListRow, and ComposeFrame's bar do not set a width or `align-self` on their root, and none of them set `width="100%"`. They lay out as max-content under the cell, not as the full column a caller shows.

They do not collapse to zero. A `flex=1` child is `flex-basis: 0%` (`contract/lower/src/values.rs:1220`), and that percentage is treated as content while the row's main size is indefinite (`vendor/taffy/src/compute/flexbox.rs:1130`), so PersonRow's `flex=1` `min-width=0` link still contributes its text and avatar. The row ends up as wide as that content: the trailing slot sits beside the name, and PostCard's surface and divider stop at the text. A root `width="100%"` would still fill this cell, because the cell column itself is stretched to a definite width and stretch is not what resolves a percentage. A root `flex=1` would not: the cell is a column, so `flex` grows height. BarItem is the only library root with `flex=1`, and it does not keep the cell's width.

Colour roles do not have a matching defect. Rank uses `style_sample`, which already walks child components, so a colour that exists only through a child still takes a slot in declared order. A variant literal replaces that prop's sample in `args` and later colour props still count it, so those placeholders stay distinct. All four names are real roles with Apple and neutral pairs (`label`, `background`, `system-blue`, `secondary-label`); caret, accent, and press rows accept them — the Inks cell would be refused otherwise, and the test asserts it is not.

VERDICT: findings
