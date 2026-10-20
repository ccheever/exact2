# Code review: stage 3 commit 6 follow-up: AccentColor offer and the wire tests in the async lane, round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `6fd2469ea1152e1d8606cb27529a21120ef23202e435cbaa06827d1c2b2fd042`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both: a role offer that varies by an app axis fails tamber-role-axis).
- **Disposition:** fixed in the first-round commit (varies, a compile-level test)

---

I'll review the last two commits read-only, starting with the review guidance and the diffs for the glue-step test and the shadcn importer change.The request is a read-only review of `HEAD~1` and `HEAD`, so I’ll inspect those diffs and the surrounding glue, importer, and test code without changing the tree.The diffs are small. Next I’ll trace how glue tests are launched and how `AccentColor` is resolved, checked, and offered.The glue list and the accent offer are the two changes. I’ll read the lock, role resolution, and review-item paths they depend on.The glue-step commit is fine. The accent offer is not contract-clean once a shadcn theme class makes `shadcn.primary` vary.

## Findings

[P2] Do not offer `AccentColor = shadcn.primary` when that token varies by a theme axis — `tamber/src/import/shadcn.rs:349`

`offers` always writes the role as the token name, with no `when` lines of its own. On the blue-class fixture, `shadcn.primary` still has `when theme=blue`, and the checker resolves that reference per tuple, so the folded accent colour differs between `default` and `blue`. Contract then refuses the design with `tamber-role-axis` (`contract/lower/src/theme.rs:252`). `lower-theme-tint` does not fire: the folded value is a `light-dark()` pair, not a tint role. `accepted()` in `tamber/tests/import.rs` only calls `exact_tamber::check`, which does not know `tamber-role-axis`, so `a_further_shadcn_theme_class_is_a_proposed_axis` stays green. The same hole already exists for the other four role overrides on that fixture; this commit puts the tint on that path, including when those four are declined and only the accent is accepted. The neutral fixture has no theme axis, so it still lowers.

## Assessment

Nothing else blocks.

`960e3b346` only adds `tamber-server` to `glueTests`. `deploy-paths.test.mjs:124` asserts containment of `app-workspace`, not a frozen list, and no other list or line cap names these files. Under `bun test --parallel` the file is one worker: the `contract tamber serve` child locks a temp app’s `.exact/tamber.lock`, and the theme-fixture dev loop is the only glue file that serves that app (`apps/*/.exact/` is gitignored). It does not edit checked-in sources, so it does not belong in the post-parallel `EDITS_APPS` pass.

On `f9ae35d1b`, `AccentColor` is a system role. `edit::role_key` / `token_index` match any case (`accentcolor` included) and `SetToken` keeps the existing spelling, so a re-import updates rather than duplicates. After a full accept, an unchanged re-import offers nothing. The wire numbers the new op as item 27 and the partial accept of items `0..23` drops it with the other overrides. `Offer.note` is unused by every importer and is still emitted as null; the shadcn header matches the new op.
