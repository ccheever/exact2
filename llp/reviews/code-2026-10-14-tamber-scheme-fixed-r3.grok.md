# Code review: LLP 1121 stage 2: scheme=fixed, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `5bbf2d9ec97ce37ed65aa24240a476444a794b2942c3ad009ddec167a694fde2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, one low; Grok: one high, three medium, one low.
- **Disposition:** applied without re-review except a row's own expression over a fixed token, and two sources at once, named

---

## High

**A profile colour can silence every one-scheme warning in the file** — `contract/lower/src/theme.rs:694`

`one_scheme_tokens` rewrites any value containing the substring `color(--` to `light-dark(#000000, #ffffff)` and leaves `scheme=fixed` in place. The real build does not do that. `profiles` stands a whole-value profile colour in as `transparent` (alpha 0, so `scheme=fixed` is legal) and matches `color(--` case-insensitively.

Two designs that compile then fail this extra fold, and the `Err` branch returns no tokens at all:

- `brand = color(--dci-p3 1 0 0) scheme=fixed` plus `white_100 = #ffffff`. The stand-in pair disagrees with the mark, `check_design` fails, and `white_100` is never reported.
- `Color(--dci-p3 1 0 0)` (capital C). The substring check misses it, the crate rejects `--dci-p3`, and again nothing is reported.

Use the same stand-in as `profiles` (`transparent`, case-insensitive, whole value and whole-name aliases). Do not keep `scheme=fixed` on a fabricated pair. A fold error should skip that token, not the rest of the file.

## Medium

**Alias exemption does not follow role identity or `when` branches** — `contract/lower/src/theme.rs:708`

Exemption is “the trimmed default text case-folds onto a fixed name”. `same_name` in this file already treats `label` and `-exact-label` as one role. This check does not.

- Declaring `-exact-label = #101010 scheme=fixed` and `ink = label` still warns. The crate resolves `label` to that token; the fixed set only contains `-exact-label`. The tested direction (`label` declared, `ink = -exact-label`) happens to work because both spellings are pushed.
- `ink = label` with `when mode=b: #ffffff` is dropped entirely, including mode `b`. Only the default string is consulted, so an unmarked solid colour on another branch is silent.

Exempt a token only when every branch (the default and each `when`) is a whole-value alias, compared with `same_name`. Judge any other branch by its fold.

**A row expression over a fixed token is not folded** — `contract/lower/src/theme.rs:684`

Lowering adds those expressions as synthetic tokens (`theme.rs:184`) and folds them. `one_scheme_tokens` never does. The warning only records whitespace-separated names (`warnings.rs:953`) and then looks those names up in the design fold.

`base = #ffffff scheme=fixed` and `background-color="color-mix(in srgb, base 50%, #000000)"` is one grey in light and dark. `base` is exempt, the `color-mix` is not a literal colour, and there is no warning. Before this change the row walk still pulled `#ffffff` out of `base` and warned. The same mix written as a design token is reported; the test covers only that form.

Fold the same synthetic list the build folds. If the result is one opaque colour, warn at the row and offer a `light-dark` pair. There is no `scheme=fixed` on a row expression.

**The summary line offers `scheme=fixed` when that mark is refused** — `contract/lower/src/warnings.rs:1259`

Each token line is right: some-modes tokens get a pair only, and an inline theme is told to move to `app.tamber`. The summary is not. With two or more token sites it always appends `{mark}`, including when one token is only one colour in some modes. The mixed summary hardcodes “mark it `scheme=fixed`” (`warnings.rs:1274`), including for an inline theme. The checker rejects that mark on a token that still has a real pair, which is what the some-modes line just said.

Build the summary from the sites actually present: a pair for a some-modes token, the inline wording when `tokens.design` is false, and the mark only for a design token that is one colour in every tuple. `tamber_scheme.rs` only asserts the single-site some-modes line, so the summary is untested.

## Low

**With both an inline `theme` and `app.tamber`, the warning reads one source and the colours of the other** — `contract/lower/src/theme.rs:685`

The build reports `tamber-two-sources` and still folds the design file. `one_scheme_tokens` does the same. `theme_decl` (`theme.rs:614`) and `Tokens.design` (`warnings.rs:133`, `theme.is_none()`) still follow the inline theme. Rows name the design tokens; the warning lists inline entries, attaches design colours to those names, and says an inline theme cannot mark a token that lives in the design file.

Drive the entry list, the spans, and the mark wording from whichever source `one_scheme_tokens` folded.

Nothing else at high, medium, or low. The one-scheme lines stay outside the 20-warning cap on purpose (`warnings.rs:30`), and the new fold is bounded by the crate’s tuple cap. Inline bare roles still fold to a host colour and are not warned. A paint-role override is still reported with no row naming it. An unmarked `light-dark` of two equal colours stays silent because it is `Folded::Pair`; that matches the stated `Folded::One` rule, and the mark is still accepted.
