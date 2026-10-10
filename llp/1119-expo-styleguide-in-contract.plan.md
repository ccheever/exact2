# LLP 1119: The Expo styleguide in Contract

**Type:** Plan
**Status:** Draft, 2026-10-09. Nothing here is built, and the gates in §7 are open.
**Systems:** see each LLP section in §3; the conversion itself is a generator and a Contract library (§4)
**Author:** Claude (Opus 5.5) for James Ide
**Implementer:** unassigned
**Date:** 2026-10-09
**Related:** LLP 1117 (custom properties), LLP 1118 (class lists), LLP 1095 D4/D10/D11 (r4), LLP 1081 D5/D7 (amended), LLP 1069.011.001 D7/D18 (r4), LLP 1053 G4 (`slashed-zero`), LLP 1116 D9 (on its own branch), LLP 1115 (write the web, ship the platform); [expo/styleguide](https://github.com/expo/styleguide) (`packages/styleguide`, `packages/styleguide-base`)

## 1. Consumer and scope

James (2026-10-09) asked for Expo's styleguide in Contract on three terms:
- **Convert it to Contract**, rather than run Tailwind or ship its CSS.
- **Keep the semantic relationships between styles.** `--expo-theme-text-secondary` stays `var(--slate-11)`. Nothing is precomputed to hex.
- **Buttons are native.** The styleguide's React `Button` is not carried over. Its look is applied to Exact's native buttons, and the platform's own touch, highlight, focus and hover show through.

This plan lists what that takes and where each part is specified. The specification is in the documents themselves: each section below names an LLP decision, and that decision's text is the requirement. The diff that adds this plan also amends those documents.

## 2. What the styleguide is

- **`expo-theme.css`:** 151 custom properties, 155 `var()` references.
  - Radix colour scales (slate, green, amber, red, blue, orange, purple, pink), light and dark, imported from `@radix-ui/colors`
  - semantic tokens: background, icon, text, border
  - component tokens: five button variants, each with background, border, hover, icon, text and disabled states
  - shadows `xs` to `xl`, light and dark
  - `.dark-theme` redefines the semantic layer; `prefers-color-scheme` covers one background
  - four dark tokens are relative colours (`hsl(from var(--red-8) h calc(s + 15) calc(l - 10))`)
- **`tailwind.js`:** a preset. It has:
  - colour utilities over the semantic layer
  - radii `xs`–`3xl`, shadows over the theme's, `heading-*` and text sizes with line height and letter spacing
  - `icon-*` sizes, `eyebrow` (JetBrains Mono, uppercase), `variant-numeric-*`, `asset-shadow` (`drop-shadow` filters)
  - six keyframe animations
  - the `hocus:` variant (hover and focus-visible), five breakpoints, `@tailwindcss/typography`
- **`@expo/styleguide-base`:** the same palette, themes, spacing (base 16) and breakpoints as JS objects, for React Native.
- **Components:** `Button`, `Link`, the theme provider. Only their looks matter here.

## 3. The work, by specification

| # | Where it is specified | What it gives the styleguide | State |
|---|---|---|---|
| 1 | **LLP 1117** D1–D8 | Tokens as custom properties: declared in the theme, `var()` in any row and in `style` blocks, overridden per subtree, resolved per subtree by the kernel, written as CSS on the web, chains reported to the agent | new, proposed |
| 2 | **LLP 1095 D4** (r4) | The `theme` block is where tokens are declared; `use theme` imports Expo's; role overrides unchanged | amended, proposed |
| 3 | **LLP 1095 D10** (r4) | `hsl(from var(--red-8) h calc(s + 15) calc(l - 10))` and the other three relative tokens | amended, proposed |
| 4 | **LLP 1095 D11** | The theme as a `:root` rule beside the role sheet | amended, proposed |
| 5 | **LLP 1081 D5, D7** | `--name` is the author's own spelling; `--exact-*` stays reserved; the lexer reads `--` names | amended, proposed (1081 is accepted; this needs its own ruling) |
| 6 | **LLP 1118** D1–D5 | `class=[HeadingSm, TextSecondary]`, later wins; styles defined from styles | new, proposed |
| 7 | **LLP 1069.011.001 D7** (r4) | A native button's `accent-color`, `color` and stroke from tokens, still dynamic per trait set | amended, proposed |
| 8 | **LLP 1069.011.001 D18** (r4) | The `tertiary` (outlined) variant as a native `plain` button with a stroke | amended, proposed |
| 9 | **LLP 1053 G4** | `slashed-zero` (`.variant-numeric-slashed`) | amended, proposed |
| 10 | **LLP 1001** | The deviations each of 1117, 1118 and 1095 r4 declares, written when each lands | at landing |
| 11 | **LLP 1017.000 P6, P8; LLP 1006** | Transcriptions of `class=` lists and `use theme`, written when the code lands (1017.000 holds only what is built) | at landing |
| 12 | **LLP 1116 D9** (author: James, branch `wip-1116-final`) | Re-expressed as four host-registered custom properties over 1117's machinery; no behaviour change | when both land |

**Already on `origin/main`, nothing to do:**
- `box-shadow` lists with spread and inset (LLP 1077 D4): every shadow token, including the two-layer ones
- `color-scheme` per subtree (LLP 1034 §8): `.dark-theme` scopes
- CSS named colours, `hsl()`, `hwb()` and wide colours (LLP 1100)
- grid tracks (LLP 1001; `grid-template-columns` and the rest are Contract attributes)
- `cursor` and `user-select` rows
- `text-transform`, `letter-spacing`, ratio `line-height`, `tabular-nums`, `line-clamp`
- `keyframes` and `font` declarations through `use` (`contract/cli/src/sources.rs`)
- native buttons with `buttonStyle`, `accent-color`, `-exact-control-size`, `-exact-corner-style`, `border-radius` and `padding` as content fields (LLP 1069.011, 1069.011.001, LLP 1104)
- `exactViewport()` widths for breakpoints (LLP 1039)

## 4. The conversion

A generator reads the styleguide's sources and writes Contract. It keeps the styleguide's own names and every `var()` link.

**`theme.contract`, the tokens.**
- Radix scales: one entry per step, light and dark merged into a pair. Radix defines a dark step under the same name, so the name is unchanged: `--slate-12 = light-dark(#1c2024, #edeef0)`.
- The semantic and component layers: as written, as chains. Where the `.dark-theme` block maps a token to a different reference, the entry becomes a pair of references (LLP 1117 D4, pairs in a pair's half): `--expo-theme-button-primary-background = light-dark(var(--expo-color-black), var(--expo-color-white))`.
- Shadows: light and dark differ only in each shadow's colour, so each colour becomes a pair and the geometry is written once: `--expo-theme-shadows-xs = 0 1px 3px light-dark(rgba(0,0,0,0.025), rgba(0,0,0,0.3)), 0 1px 2px light-dark(rgba(0,0,0,0.05), rgba(0,0,0,0.3))`. The generator refuses any token whose light and dark geometry differ, so that case can't pass silently.
- Spacing, radii, sizes and durations from `tailwind.js` and `styleguide-base`: `--space-4 = 16px`, `--radius-md = 6px`, `--duration-default = 150ms`.

**`styles.contract`, the utilities and components, as `style` blocks over the tokens.**
- Colour, radius and shadow utilities, one style each: `style BgDefault background-color="var(--expo-theme-background-default)"`.
- `heading-*` and the text sizes: font size, weight, line height and letter spacing per style.
- `Eyebrow`: JetBrains Mono as a declared `font` (TTF; WOFF2 is refused, LLP 1019) and `text-transform=uppercase`.
- `IconSm` … `Icon2xl`.
- The six keyframe animations as `keyframes`.
- Button variants as native-button styles (LLP 1118 D3):

| Variant | Style |
|---|---|
| primary | `buttonStyle="filled" accent-color="var(--expo-theme-button-primary-background)" color="var(--expo-theme-button-primary-text)"` |
| secondary | `buttonStyle="gray" color="var(--expo-theme-button-secondary-text)"` |
| tertiary | `buttonStyle="plain" border-width=1 border-color="var(--expo-theme-button-tertiary-border)" color="var(--expo-theme-button-tertiary-text)"` (D18) |
| quaternary | `buttonStyle="plain" color="var(--expo-theme-button-quaternary-text)"` |
| primary destructive | `buttonStyle="filled" accent-color="var(--expo-theme-button-primary-destructive-background)"` |

- Each variant also takes `-exact-corner-style` or `border-radius="var(--radius-md)"`, by the styleguide's size.
- The hover tokens and the disabled colours are not carried: the platform draws highlight, hover and disabled (LLP 1069.011.001 D16; LLP 1115).

**Not converted:**
- **`hocus:` on anything but a button.** A row that changes on hover is a `hover` handler and a state-bound class, as on any Exact node. There is no pseudo-class to convert to, and the native button already has the platform's hover.
- **`@tailwindcss/typography`.** Its prose styles are selectors over descendants. Exact's Markdown reader has its own block styles (LLP 1033).
- **Tailwind's generic utilities** (`flex`, `p-4`, `gap-2`). In Contract these are attributes already: `padding="var(--space-4)"`.
- **`translate-z`, `backface-hidden`, `transform-box`.** These are browser compositing workarounds.
- **`asset-shadow` on Linux.** LLP 1055.000 records `filter` on a box as drawn on the web and Apple. The Linux painter (`host/linux/src/paint.rs`) has no box-filter path that I could find. That makes Linux a likely gap, to be confirmed by the lab app.

## 5. Order

1. **LLP 1117 stages 1–3** (Contract, kernel, web and JS target), with LLP 1081's amendment and LLP 1095 D4 r4. This is enough for a token theme on every host, because Apple and Linux receive resolved rows.
2. **LLP 1118** (class lists): Contract only.
3. **LLP 1095 D10 r4** (relative colour), with D11 (the role and theme sheets).
4. **LLP 1069.011.001 D7 r4 and D18**, then **LLP 1053 G4's `slashed-zero`**.
5. **The generator and the library (§4), outside this repo (§6 Q1)**, and a test app that draws every token, style and button variant in light, dark and a `color-scheme` subtree. Every screen is checked three ways:
   - against the styleguide's own `example-web` in Chrome (the web oracle for the theme)
   - against a hand-configured `UIButton` for each variant on iOS (the native oracle, LLP 1069.011.001 §4)
   - with the agent's `layout` chain report, which must name each token
6. **LLP 1116 D9** re-expressed, once 1116 and step 1 have both landed.

## 6. Open questions

1. **Where do the generator and library live?** *Decided (James, 2026-10-09):* not in this repo. Expo's styleguide is not committed to Exact. For now it is only a dependency of Exact's test apps. The generator and its output live outside `exact2` (with `expo/styleguide`, or beside the test apps). A test app consumes the generated `.contract` files by path, as an app outside the repo does (LLP 1036.001), or generates them at build time into an ignored directory. What this repo gains is only the general features in §3; nothing in it names Expo's tokens.
2. **Radix's P3 values.** `@radix-ui/colors` ships `color(display-p3 …)` variants for wide-gamut displays. Exact keeps a wide colour's space (LLP 1100). Recommendation: generate the P3 values, with sRGB as the fallback LLP 1100 already computes. Measure on an iPhone first.
3. **Breakpoints.** `xs`–`xl` and `compact-height` are media queries. In Exact they are `when` conditions over `exactViewport()`, and their numbers are logic, not CSS (LLP 1117 §4). Options: the lab app restates the five numbers; Contract gains named constants; or `styleguide-base`'s numbers come in through a data source. Recommendation: restate them until a second app needs them.
4. **Hover on links and rows.** The styleguide's `hocus:` colours links and list rows. Per LLP 1115, iPad and Mac pointer feedback is the platform's (`hover-effect`, LLP 1077 §5). Recommendation: don't carry the hover colours to native at all, and keep them on the web through a `hover` handler only where Expo's web pages need them.
5. **Which buttons stay custom?** A variant that needs a fill no style draws, or per-side borders, stays `appearance="none"`. None of the five does with D18. Confirm against expo.dev's real buttons.

## 7. Gates

- **`rules/DEFERRED.md`.** Custom properties and class lists are not on the doing list. Moving them there takes a written reason and something taken off, or Charlie's waiver (RULES, "Scope"). This plan is the reason; the take or the waiver is Charlie's.
- **An implementer and a date** for each of LLP 1117, 1118 and the r4 amendments (RULES: a spec needs both before it is built).
- **Review.** LLP 1117 and 1118 go through the LLP 0005 loop before acceptance, as LLP 1095 and 1069.011.001 did.
