# Code review: @exact/cards ProfileHeader (stage 13 C piece 2) and the gallery's typed call arguments, round 1, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `80fd041776`.
- **Method:** one brief (sha256 `aa492c3b0079dd825e4ee5b8d8d759a07510731b5186bbf30af5302cb9b725e9`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** both taken in e4a06e5ce3 (argument names by position; the fade, solved in round 2)

**VERDICT: RIGHT WITH NITS**

The two enum deviations are not problems. `ProfileAlert.icon` is the clone’s four glyphs (`warning`, `eyeslash`, `ban`, `info`); theme2’s `tone: warn | info` cannot pick among them. Every pill uses the same ink and ground, both params whose defaults are roles (`cards.alert`, `cards.alert-ground`). `badge` is `none | verified | verifier` because `VerifiedBadge` draws those two strings (the verifier’s accessible name is “Trusted verifier”; the value is not `trusted`). `none` only skips the badge. It is not a D5 variant, because it is read by `if`, not by a class or a style `?:`.

D0–D5 hold. The entry’s library component is `ProfileHeader`. `ProfileAlert` is a shape, so the compiler’s export walk does not treat it as a component; callers need the name to build the list. The view is banner, avatar, then the actions slot, names, alerts, counts. `children` sits once on part `actions`, outside any `if` / `for` / `match`, and is not forwarded. Nothing on that chain is `display: none`, zero-sized, or `aria-hidden`. The alerts row is `display: none` when `alerts` is empty, so its padding does not add a gap. `blocked` hides the counts column (and the note inside it). `compact` only shortens the banner and moves the avatar by the same 50px. Those two bools are the only variants, so the gate’s three cells plus the badge, follows-you, alerts, and note samples match the rule. Resting `pull` is scale 1, so it does not move the ada/bo shots. Bio, “Followed by”, the back button, and the pull spinner are not in the shell.

`contract gallery` is the only caller of this typing. Nothing in `contract/lower` or `contract/types` changed, so the 46-app compile path is the same. A `for` item’s `string` / `number` / `bool` field is typed as itself. An enum field becomes `string`, and `Ty::unify` accepts that, so `openAlert(a.id)` still places. A bare forward stays `noop`.

1. **should-fix** — A fifth explicit action argument is declared under the same name as the fourth, which fails to parse and aborts the whole gallery. `contract/cli/src/gallery/inert.rs:96` (the cap is `ARGS` at line 116). `i.min(ARGS.len() - 1)` names arguments `a`, `b`, `c`, `d`, then `d` again. Duplicate action parameters are a parse error, and that declaration sits on the gallery component, not in a cell, so `generate` ends in “the gallery does not compile” instead of refusing the one cell. `pick("x")` and a bare forward are tested; five arguments are not.

2. **nit** — The status-bar fade’s height is a raw length. `packages/cards/profile-header.contract:76`. `calc(env(safe-area-inset-top) + 20px)` is the only non-zero length written in a library view. The other profile sizes, including the 2px and 6px ones, are `cards.*` tokens. `height` is not a `lower-library-literal` row, so the checker will not ask for a token.
