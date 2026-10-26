# Code review: @exact/cards ProfileHeader (stage 13 C piece 2) and the gallery's typed call arguments, round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e4a06e5ce3`.
- **Method:** one brief (sha256 `f30f7289f3edb2342e7337fc394520447a7afe9b0f448684a415a5efc629fd00`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed

**VERDICT: RIGHT**

No findings.

The piece-2 diff against `1b7936f89e` is the gallery inferencer, `@exact/cards`, the library gate, and one `QUEUE.md` line. `contract/lower`, `contract/syntax`, and `contract/types` are unchanged. `gallery.rs` only passes the merged file into `Inert` (`contract/cli/src/gallery.rs:110` and `:999`).

Each round-1 answer holds:

- Call arguments are `a0`–`a7` (`contract/cli/src/gallery/inert.rs:123`). Index 8 makes that argument `None`, and `collect` then drops the whole signature to `noop`. Payload names (`checked`, `value`, `scrollLeft`, …) do not collide with those. `Five` (`contract/cli/tests/it/gallery.rs:760`) is `press=pick(1, 2, 3, 4, \`e${1}\`)`, and the fixture expects `(a0: number, a1: number, a2: number, a3: number, a4: string)` at `:845`. Only `Forward` is refused.
- A `for` resolves its list in the outer scope, then stores the element type on the binding (`inert.rs:191–225`). `Word[][]` is a list of lists, so `ShadowList` (`gallery.rs:773`) types `x.id` from `Word`, not from the hidden `items: Num[]`. The cell is placed with `pick=noop_string` (`:846`).
- `badge` and `badgeMark` are color params (`packages/cards/profile-header.contract:26–27`). `VerifiedBadge` takes `ink` and `mark` (`packages/cards/common.contract:64–65`) and writes them to `fill`, which is a style row (`contract/lower/src/style_names.rs:40`). A `Use` argument is visited as a row value (`contract/lower/src/params/rewrite.rs:357–360`), so both params are reached. `PostCard` still passes `cards.badge` and `cards.badge-mark` (`packages/cards/post-card.contract:99`).
- The fade stays `height="calc(env(safe-area-inset-top) + 20px)"` (`profile-header.contract:80`). Height is not a D1 knob. A view `calc()` accepts `env()` and px, and an identifier there is only `e` / `pi` / `infinity` / `nan` (`kernel/src/style/calc/parse.rs:353–362`), so a token name is `lower-attr-value`. A length token’s `calc()` does not take `env()` (`tamber/src/measure.rs:572–619` falls through to `tamber-syntax`). `env(…)` is not a token, role, or base, so as padding it is a D1 literal (`contract/lower/src/anatomy/library.rs:12–40`, `:87–101`). The `QUEUE.md:3` entry matches that.
- `cards.banner-shade` is `#00000080` `scheme=fixed` (`packages/cards/cards.tamber:34`), used only as the gradient over the banner picture (`profile-header.contract:81`). The platform role list has no scrim. Package tokens are not judged by `lower-colour-one-scheme`, and a translucent colour would not be reported anyway (`contract/lower/src/theme/scheme.rs:25–26`, `:63–66`).
- `SLOTTED` is `PostCard`, `ProfileHeader` (`scripts/stage12-gate.mjs:519`). Those are the two exports that declare `slot`.
- `cards.avatar-top` is `calc(cards.banner-height - cards.avatar-overlap)` = 104px, the compact one is 54px, and `cards.profile-avatar-box` is `calc(cards.profile-avatar + cards.profile-avatar-ring * 2)` = 94px (`cards.tamber:103–108`). `*` binds tighter than `+`. `cards.banner` aliases `cards.avatar-default` (`:31`, `:20`), a whole alias of a `scheme=fixed` colour.

D0–D5 after the change: `ProfileHeader` is the exported component; `ProfileAlert` is the shape, with icon `warning` / `eyeslash` / `ban` / `info` and badge `none` / `verified` / `verifier`. The variant axes are `compact` and `blocked` (row ternaries). `badge`, `followsYou`, and `note` are structural `if`s, so the gallery adds three cells (first values, then each other value alone, `contract/cli/src/gallery.rs:31`), and the gate expects 7 components and 16 cells. `children` sits once on `part="actions"` (`profile-header.contract:117–123`). The banner `overflow` and the alerts/counts `display` ternaries are off that chain. Part groups constrain class rows on the part node (`contract/lower/src/anatomy.rs:429–441`); these parts bring no out-of-group class.

This pass did not re-run the gallery tests or the web copy check. The token arithmetic above is what keeps the default banner, avatar box, and overlap the same as the old literals.
