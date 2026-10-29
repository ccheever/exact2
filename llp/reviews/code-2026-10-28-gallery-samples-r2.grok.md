# Code review: gallery: distinct colour samples (D4 batch 2 #5; #4 taken back), round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `dc33c1da96`.
- **Method:** one brief (sha256 `573803583235aa3996689ea4982939f8fe0c5b89f551c76a1ad660f24b2558e0`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the gallery fix as a read-only pass on those two commits, starting with the diff and the sizing, colour-sample, and ranking code.The two commits are the gallery sizing and colour-sample changes. I'll read those functions and how colour roles are defined on each host.**SHOULD** `contract/cli/src/gallery.rs:1033` — `sizes_itself` is true only when the first view node is an element that writes `width` or `align-self`. `PostButton`'s root is a `button` with neither (`packages/actions/post-button.contract:35`), and its look is a pill (`radius.full`, horizontal padding, no width). The gallery cell is a column, so the default stretch makes that pill as wide as the cell. In the app it sits in a row (`packages/actions/compose-frame.contract:55`) and stays as wide as its label. That is the same miss as FollowButton's pill. FollowButton is exempt only because its root writes `width`.

The rest of the pass is clean.

A root `width` that resolves to `100%`, or to any other definite length, keeps that size when the cell sets `align-items: flex-start`. Stretch replaces an automatic width only. FollowButton's expression (`"auto"` or a fixed token such as `actions.follow-round` = 33px) is handled because the attribute is present. `actions.header-slot` is 33px, so `HeaderButton` is the same.

A root `Use` is inlined to the callee's root, and a root `if` or `match` contributes the active arm's element as the flex item. The library roots of that shape write a definite width: `VerifiedBadge`'s arms use `width=size`, the standard-library `RoundButton`'s arms use `width=80`, and the test's `Ring` inlines `Dot`'s `width=size` (sampled as `24px`). Those keep their size without `flex-start`. `ListRow`'s arms are full-width cells and correctly still stretch. An arm or callee whose width is `auto` would stretch. No library component is shaped that way.

`max-width` alone leaves the width automatic, so the cell stretches and the cap clamps it. That is how a default column already lays out `EmbedView` and `PollView`.

The four samples are real roles in the schema table, with fallbacks on every host. The web writes `var(--exact-…, <fallback>)`. Apple resolves `labelColor`, `secondaryLabelColor`, `systemBlueColor`, and `systemRedColor`. Linux uses the neutral column (black, `#474747`, `#0b66d6`, `#c90f1a` in light). Android's Material map sends them to OnSurface, OnSurfaceVariant, Primary, and Error (`host/linux/src/presenter/palette.rs:128`). All four read on the page ground in light and dark. The terminal resolves the same roles to RGBA.

`sample` does not panic on an empty free list. `rank % free.len().max(1)` avoids a remainder of zero, and `free.get(…).unwrap_or` returns `-exact-label` (`contract/cli/src/gallery.rs:1067`).

VERDICT: findings
