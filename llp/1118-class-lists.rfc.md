# LLP 1118: Class lists — composing named styles

**Type:** RFC
**Status:** Draft r1, 2026-10-09, for review. Nothing here is built.
**Systems:** Contract (`contract/syntax`: the list form of `class=` and `class=` in a `style`; `contract/lower/src/class.rs`: the merge; `contract/lower/src/controls.rs`: the native-button rules over a merged list); web and JS targets (one class per style in `app.css`, unchanged otherwise)
**Author:** Claude (Opus 5.5) for James Ide
**Implementer:** unassigned (RULES: a spec needs an implementer and a date before it is built)
**Date:** 2026-10-09
**Related:**
- LLP 1017.000 P6 (named styles and `class=`, as built), P8 (`use`).
- LLP 1069.011 D1, D12 (`appearance` and styleable props through a class).
- LLP 1117 (custom properties): a style can carry `--name` declarations.
- LLP 1119 (the Expo styleguide plan): the first consumer.
- HTML's `class` attribute; Tailwind's `@apply` and Expo's own `mergeClasses` (`tailwind-merge`, where the later class wins).

## Summary

`class=` takes one style, or a choice between two (LLP 1017.000 P6). A design system is many small styles used together: Expo's styleguide writes `heading-sm text-secondary` on a label and `rounded-md shadow-xs bg-default` on a card. Today an app has to declare one style per combination, so the names and the relationships between them are lost.

This RFC does two things:
1. **`class=` takes a list**, as HTML's attribute does: `class=[HeadingSm, TextSecondary]`.
2. **A `style` may itself use `class=`** with a literal list, so a style can be defined from others (`style ButtonPrimary class=[ButtonBase, ShadowXs]`).

Both expand at compile time, as P6 does. There is still no selector, no cascade and no runtime class machinery: the kernel sees rows.

## 1. Motivation

- **Combination explosion.** With a single class, `heading-sm` × eight text colours is eight styles, and every new colour multiplies. With a list it is nine styles.
- **Relationships.** `ButtonPrimary` is `ButtonBase` plus a colour. Declared as a copy, a change to `ButtonBase` has to be repeated in every variant.
- **The web is the standard.** HTML's `class` is a list; P6 took the attribute's name and one value.

## 2. Decisions

### D1 — `class=` takes a list

```
text class=[HeadingSm, TextSecondary] "Projects"
column class=[Card, ShadowXs, (selected ? BgSelected : BgDefault)]
```

- **Elements:** a style name, or a parenthesized choice `(cond ? A : B)` whose arms are names. A single name and P6's single choice remain valid without brackets.
- **`none` as an arm:** `(selected ? BgSelected : none)` applies nothing in that arm. This is HTML's conditional class.
- **Duplicates** (one name twice in a list) are `lower-class-duplicate`.
- **Refused:** a computed name (`type-class-name`, as P6), and nested lists.

### D2 — Later wins, the node's own attributes win over all

- **For each row, the last list element that sets it wins, and the node's own attribute wins over every element.** P6's rule ("its own attribute of the same name replaces the style's") holds unchanged.
- **This is a declared deviation from CSS**, where the stylesheet's order decides and the attribute's order doesn't. Contract has no stylesheet order: styles come from several files through `use`, and declaration order across files isn't something an author sees. List order is local and readable. It is also what Expo's `mergeClasses` (`tailwind-merge`) does, so a converted design system keeps its precedence.
- **A conditional element** contributes its chosen arm's rows. For a row that only one arm sets, the other arm falls through to the earlier elements. The compiler folds each row into a conditional over the list, as P6 already synthesizes one for a single choice. So `class=[Card, (dense ? Tight : none)]` with `Tight` setting `padding=8` gives padding `dense ? 8 : <Card's padding>`.

### D3 — A style may be defined from other styles

```
style ButtonBase
  -exact-control-size=large -exact-corner-style=medium

style ButtonPrimary
  class=[ButtonBase]
  buttonStyle="filled" accent-color="var(--expo-theme-button-primary-background)"
```

- **`class=` inside a `style`** takes a literal list of names: no conditions, since a style is constant (P6's `contract-style-literal`). The style's own lines win over its list, as a node's attributes win over its classes.
- **Expansion is at compile time and transitive.** A cycle is `lower-class-cycle`.
- **Across files:** a used style brings the styles it lists, as a used component brings what it needs (P8).
- **This has no CSS equivalent.** CSS composes at the use site only; `@apply` and Sass's `@extend` are tools over CSS. It is admitted because `style` is Contract's own construct, and a design system's styles are defined from each other (§4 Q1).

### D4 — The web output is unchanged in kind

Each style is still one class in `app.css`. A node's list becomes its `class` attribute in list order. Since the browser decides by stylesheet order, not attribute order, the build orders the rules so the browser agrees with D2. Where two lists order the same pair of styles differently, it writes the losing rows inline on the node instead. The conformance run checks both cases.

### D5 — Native buttons and styleable props

- **`appearance`** must still be a literal after the merge (LLP 1069.011 D1). A list whose conditional element changes `appearance` is refused (`lower-button-appearance`), as a bound one is.
- **Styleable props** (`buttonStyle`, LLP 1069.011 D12) merge as rows do. P6's both-arms-or-neither rule (`lower-style-prop`) is relaxed: an arm that doesn't set the prop is allowed when an earlier element sets it, because the fallback is then defined.

## 3. Cost

About 150 lines in `syntax` and `lower`, most of it in `class.rs`'s merge. There are no kernel, wire or host changes, and no new runtime work: the merged rows are what P6 already lowers.

## 4. Open questions

1. **Admit D3 (styles from styles)?** It is the one part with no web equivalent. Without it, `ButtonPrimary` is written at each use as `class=[ButtonBase, ButtonPrimaryColours]`. Recommendation: admit it. Styles are already Contract's, and the alternative pushes a design system's structure into every call site.
2. **List order or declaration order (D2)?** Declaration order would match CSS exactly within one file, and is ill-defined across `use`. Recommendation: list order, declared in LLP 1001.
