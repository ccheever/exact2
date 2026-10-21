# Code review: Stage 12 E follow-up: PostButton's pressed and dim params, the packages' sizes and type as tokens, neutral colours as roles (LLP 1139 §5 E), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `33083088ec`.
- **Method:** one brief (sha256 `20e8865fdcd985444d18075fe7e874d698615806f37ad5e0ebcacda7a093d166`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocker, no should-fix; two nits (the terminal ring; the test misses a fill/dim swap and pressed)
- **Disposition:** both taken (as Astra's)

No blockers and no should-fixes. Two nits.

## NIT — On the terminal, `radius.full` is not the circle `size / 2` was

`packages/cards/common.contract:32` and `:46` set the avatar and its ring to `border-radius="radius.full"`. On web, iOS, macOS, and the Linux painter (which the Android host shares), that is the same circle as half the side: the base cell is 9999px (`kernel/src/style/scale.rs:28`, `kernel/tables/schema.json:6001`), and each of those hosts reduces an oversized uniform radius to half the border box before painting (`host/linux/src/paint/border.rs:85`, used from `host/linux/src/gpu.rs:432`; `host/apple/Sources/ExactKit/BorderPaint.swift:69`, applied before the Core Animation half-side check in `host/apple/Sources/ExactKit/IOS/BoxLayerIOS.swift:235`). Web is the browser’s own CSS Backgrounds 3 corner reduction of `9999px`.

The terminal cell of every radius, including `radius.full`, is 0 (`kernel/src/style/scale.rs:268`, `host/terminal/src/host_scale_tests.rs:34`). The terminal draws a round corner glyph only when some radius resolves to points greater than 0 (`host/terminal/src/paint.rs:400`). A 21px or 16px radius did; 0 draws `┌`. Overflow still clips to a rectangle (`host/terminal/src/paint.rs:239`), so the photo was never circular — only the hairline ring’s corners change. That matches the scale rule that the terminal draws no corner. An app that sets `radius.full` to a px length wins on the terminal too.

## NIT — The new test locks the disabled-row bug, and would still pass if `fill` and `dim` were swapped

`contract/cli/tests/it/library_packages.rs:87` compares the two buttons with `assert_ne`, and `:88` and `:89` compare each to the package defaults. A no-op override fails those shipped comparisons, and a missing test id panics in `background` (`:48`), so the test does not pass vacuously for the `:disabled` bug it names. It never checks that the enabled button is `#ff0000` and the disabled one is `#00ff00`, so wiring the two params to each other’s buttons still passes.

`pressed` is untested. `:active` lowers to `-exact-press-background-color`, not the resting background (`contract/lower/src/states.rs:11`, `packages/actions/post-button.contract:14`), so this boot-time read cannot see it. `PostButtonLook` is applied only by `PostButton`, so `param.pressed` is rewritten to that param’s token the same way `param.dim` is (`contract/lower/src/params/rewrite.rs:514`). `scheme=fixed` on the test’s params lines is metadata and is dropped when the param token is laid down; the hex value is what applies.

Temp-dir cleanup is right. `Dir`’s `Drop` removes the directory (`library_packages.rs:20`), `_shipped` lives until the function returns, and `app_with` deletes any previous directory first (`:30`). `packages/actions` is flat, so the non-recursive copy matches it. A panic during setup, before `Dir` is returned, would leak that one directory.

## Checked, not findings

Each new length token is the old literal, and each old icon size, font size, line height, avatar size, the 2px thread, the -1 nudge, and the 0.5 hairlines in these two packages now reads one. `cards.hairline` is the declared change to `-exact-hairline`. What remains numeric (`opacity=0.6`, `max-width=320`, `aspect-ratio`, font weights, line clamps, SVG path coordinates) was outside the ask.

A token name in a bound string reaches `svg` and `image` `width`/`height` and column `width`: those are style rows, and `set_dynamic` substitutes a length token’s text (`kernel/build.rs:1259`) before parsing `42px` as points. `border-top-width=(lineAbove ? "0px" : "cards.hairline")` is legal because border width accepts text; only the token arm is measured, then `-exact-hairline` becomes one device pixel. It is not used as a font size or line height.

Role defaults and `-exact-hairline` are legal knobs (`contract/lower/src/anatomy/library.rs:82`). `apps/library-gate` only sets `gate.ground`, and `scripts/library-gate.test.mjs` checks variants and slots, not colours or hairline width. `cards.line` and `cards.line-thread` now share `-exact-separator`; the tamber header says the clone sets every colour token itself.
