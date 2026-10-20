# Code review: Stage 12 E: @exact/cards, @exact/actions and apps/library-gate (LLP 1139 §5 E), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `8249e8c027`.
- **Method:** one brief (sha256 `4210d74f8027aba74af1257b0a9cade63723e2b811886a967b7360f6d219b36b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking; should-fix: post-pad, PostButton's aria-label, saved and context not D5 variants; nit: "Pinned" hardcoded
- **Disposition:** post-pad and contextText taken in 086a3086af; aria-label kept (Bluesky's own accessibilityLabel); saved and context as sample- cells

## @exact/actions — should-fix

D1 on the components holds: colour, padding, radius, and gap rows name package tokens (the only length literal in a component is `0px`), and every param defaults to a package token. D2 holds: `PostButtonLook` and `ActionButton` only bring paint, text, shape, and space rows, and those are the groups on `surface` and the six buttons. Neither component declares a slot. D6 holds: both button kinds are authored pressables. `background-color` is not a native-button row, so `PostButton`'s fill demotes it, and `-exact-press-haptic` demotes every action-row button even when the value is `"none"`. No `exactDelivery`, `exactSurface`, or `exactViewport` read, and no per-OS branch. D8 holds: the namespace is `actions`, and token and param names are the full identities. The shell is right for this package. Counts, toggles, and actions are plain props. The app keeps the data.

**Should-fix.** `packages/actions/actions.tamber:13` sets `actions.post-pad` to `15px`. That token is horizontal padding, and no base name is 15 (`space.md` is the grouped-row inset). The comment on line 2 says spacing aliases base names. Fix: default it to a base name, or a calc over one. The clone already overrides this token to 15px, so that adoption stays put. `actions.post-height = 32px` (`:12`) is a height, so leave it.

**Should-fix.** `packages/actions/post-button.contract:33` sets `aria-label` to `"Publish reply"` or `"Publish post"`. That replaces the accessible name, so the visible labels the clone actually shows (`Reply`, `Post All`, `Posting…`) are announced as those two strings, which exist only in this package. Fix: delete the attribute, or set it to `label`.

**Should-fix.** `saved` changes the bookmark glyph and its colour, but only as a `Glyph(...)` argument at `packages/actions/action-row.contract:125` (plus `aria-label` and `aria-pressed`). A call argument is not a style row, so the variant pass does not mark `saved`. `reposted` and `liked` are marked, because their count text reads them in `color` and `font-weight` (`:95`, `:114–115`). Fix: read `saved` in a class or a style row on the save button itself. `replyDisabled` stays a `disabled` prop, which is the right call. `haptics` is marked because `-exact-press-haptic` is a style row. That select will not move pixels. Leave it.

`actions.row-lead = -6px` and `actions.row-lead-big = -2px` (`:32–33`) are the allowed negative-length exception.

## @exact/cards — should-fix

Same D1 result on the components: param defaults are `cards.*` tokens, and the colour, padding, radius, and gap rows name tokens. D2 holds. Part names are `surface`, `avatar`, `meta`, `content` on `PostCard` and `frame`, `visit` on `AtCard`, spelled `parts` plus `part="name"`. The `Name` class sits on the name text, which is not a part, so groups do not apply to it. D4 holds for the one slot: `children` is a direct child of `column part="content"` at `packages/cards/post-card.contract:109`, not inside `if`, `match`, or `for`, and not forwarded. The chain is that column (`flex=1`, `min-width=0`), the row with `margin-top="cards.nudge"`, and the surface column. `min-width`, `flex`, padding, and `border-top-width` are not on the slot deny-list. `AtCard` has no slot. D6 and D8 hold: the card is a `link`, there is no platform read, and the namespace is `cards`.

The shell matches the ruling. Plain props for who, when, avatar, reply line, and context text. One unconditional slot for the app's body, embeds, moderation, and action row. `inThread`, `lineAbove`, and `lineBelow` as three bools is the right thread API: `inThread` changes padding and line colour on its own. Nothing the clone's FeedPost, Controls, AtCard, or PostButton calls need is missing from the prop lists.

**Should-fix.** `context` is the variant the ruling asked for, and it is not marked. It is read only as the `if` at `packages/cards/post-card.contract:51`, the `Glyph` `d=` argument at `:53`, and the text content at `:54`. An `if` condition and a call argument are not scanned, and text content is not a style row, so `describe` will not list `context`. `inThread`, `lineAbove`, and `lineBelow` are marked (`:36–41`). Fix: read `context` in a class or a style-row `?:` on a node of `PostCard`'s own view. The context text's `color` is the natural place. Keep the `if`. That row is not on the slot chain.

**Nit.** `:54` hardcodes `"Pinned"`, so the pinned cell ignores `contextText` (the gate still passes `"Reposted by Fen"` there). Fix: let `contextText` supply both lines.

`cards.context-hang = -16px` and `cards.badge-hang = -2px` (`packages/cards/cards.tamber:55–56`) are the same allowed exception. That is four negative length tokens across the two packages, not two.

## library-gate — meets §5 E

All four exports are placed, one bool or enum value at a time, others at the first value (`false`, or `"none"`). That is 6 `PostCard` + 7 `ActionRow` + 3 `PostButton` + 1 `AtCard`. `PostButton` has no D5 variant (`reply` is only the aria string, `disabled` is the attribute), and the single required placement is among the three. Each `PostCard` slot has a 24×24 marker (`apps/library-gate/app.contract:25–26` and the five siblings) whose glyph `marker-toggle` flips between `A` and `B`. Every part node takes its testId from `id`.

`scripts/library-gate.test.mjs` is sound for the three questions. `exportsOf` reads `component` and `use … from "./…"` on the entry, and line 49 pins the name list, so a new export fails before the cells are updated. `variantsOf` takes every bool and enum in the four-space `props` block. `cellsFor` builds the first-values row, then each later value alone. `placements` compares those maps, so a missing cell, an extra cell, a duplicate, or two props varied together fails. A new bool or enum prop fails the same way. The test was read, not run.

It is stricter than D5, which is why the `context` and `saved` cells are still required even though the compiler will not call those props variants until the should-fixes above. It does not check markers or part testIds. Those belong to the rendered gate. A placement must stay on one line, and a prop line must stay at four spaces, or the parse fails closed.
DONE
