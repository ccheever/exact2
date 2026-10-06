# LLP 1034: Scheme-aware colour — a row holds a pair, the host resolves it

**Type:** RFC
**Status:** Draft r2, landed 2026-09-08; §8 amended 2026-10-05, per-subtree `color-scheme` (approved by Charlie 2026-10-05: "Ok sounds good, go for it") (r1 written 2026-09-08; r2 the same day, folding a gpt-6-astra review at high effort that Charlie convened — artifact `llp/reviews/1034-scheme-aware-colour.astra.md`, dispositions there. Four of its five findings were checked against source and confirmed; one claim of r1's was false and is corrected in D6.)
**Systems:** Kernel (the style table's value vocabulary), Web host, Apple host, Linux host, Contract, Agent API
**Author:** Claude (Opus 5) for Charlie Cheever
**Implementer:** Charlie Cheever, from 2026-09-08
**Date:** 2026-09-08
**Related:** LLP 1001 §1 (the style table and its value vocabulary — this amends it), LLP 1007 (the web host computes CSS once from the rows), LLP 1008 (the Apple host's style application), LLP 1015 (the Linux painter), LLP 1002/1003 (what `transition` animates, and does not), LLP 1030 D3a (the compatibility id), LLP 1033 D6 (the consumer that found this, and the workaround it is built on), `rules/RULES.md` §Scope (the web is the standard)

## Summary

A colour style row can hold **two** colours and a rule for choosing between
them, exactly as a length row already holds an unresolved `env()`. The
authored form is CSS's own:

```
background-color=light-dark("#ffffff", "#17181b")
```

The kernel stores the pair unresolved. **Each host resolves it** — the web by
emitting the function and letting the browser do it, Apple and Linux against
the appearance they already know. Nothing about the appearance reaches the
app: no state, no resource argument, no reporting node.

This is not a new mechanism. It is the second instance of one the kernel has
had since LLP 1001: a value that is stored deferred and resolved against
something the host supplies.

## 1. Motivation

Two readers landed with a real dark palette on 2026-09-08 (LLP 1033 D6) and
paid for it in a way that does not scale.

Contract has no scheme-aware colour. Every colour in a view is a literal, so
"draw this dark" had to become data: a `shape Theme` of 21 tokens answered by
a data source, `provide`d at the root, `inject`ed by every component that
draws. That part is sound and stays (§3).

What does not scale is the second half. For a palette to *follow the system*
the app has to know what the system is, and nothing told it: `env()` carries
lengths only — four insets and the keyboard — and no host reported an
appearance. So LLP 1033 D6 added, per app, a `systemScheme` slot and a
zero-sized node with `testId="appearance"` that the host writes into; and per
host, a reading of the OS preference and a watcher for changes. macOS has
that today. iOS and the web do not, which is why "system" on those surfaces
still draws a light page under a host chrome that follows the OS.

That is one mechanism per app times one mechanism per host, for a fact the
platform already knows and the browser already acts on. And it is a fact the
app has no business holding: an appearance is the host's, the way a safe-area
inset is.

The cost is also paid by apps that want nothing so elaborate. A single node
that should be white on white and near-black on black cannot say so without a
data source, a provider, and an inject.

## 2. Decisions

### D1 — `light-dark(a, b)` is the authored form, and it is a value, not a property

CSS's own function, by its own name, with its own meaning: the first colour
when the used `color-scheme` is light, the second when it is dark
(`rules/RULES.md` §Scope — a semantic that could follow CSS follows CSS).

It is a *value*, so it is admitted anywhere a colour is: the original eight `rgba8`
rows were `background_color`, `border_color_top`/`right`/`bottom`/`left`,
`shadow_color`, `tint_color`, `text_color`.
The unused `tint_color` slot became CSS `caret_color` on 2026-09-10 (LLP 1001 §1); its optional colour admits the same pair, with `auto` kept distinct.

A colour row becomes what a dimension row already is — a small enum with a
deferred variant — rather than a second row per colour.

**The codec is new.** `kernel/tables/schema.json` does list a `color2 (rgba8
rgba8)` that no row uses, and r1 claimed that as half the work. It is not:
`Reader::color2` is `[self.color()?, self.color()?]`
(`kernel/src/wire/codec.rs:178`) — two colours back to back, with no tag. What
this needs is a *tagged* value, one byte then one or two colours, the way
`dimension` is "u8 kind + f32". `color2` stays unused; the eight colour rows
move to the tagged codec.

### D2 — The host resolves it. The kernel must not

This is the one asymmetry with `env()`, and it is the reason this design is
better than flattening in the kernel.

A length resolves *in* the kernel because layout depends on it:

```rust
Dimension::Env(Edge, f32)                                              // kernel/src/style.rs:119
Dimension::Env(edge, plus) => Dimension::Points(env.inset(edge) + plus) //           :171
```

Taffy has to see a number. Nothing about layout depends on a colour, so a
colour has no such obligation — and acquires a prohibition instead. If the
kernel flattened `light-dark()` to one colour it would have to *pick*, and the
web host would be handed a decision the browser should have made. What the
web host should emit is the function:

```css
background-color: light-dark(#ffffff, #17181b);
```

`color-scheme` is an inherited CSS property and `glue.js` already sets it on
the document element from `setScheme`. So the browser resolves per element,
with no JavaScript, no repaint pass of ours, and no appearance plumbing at
all. That makes the web the parity oracle here, which is the shape LLP 1001
wants and the shape layout already has.

**The initial scheme has to be established before the first pixel, and today
it is not.** `glue.js` sets `color-scheme` only when a `setScheme` command
arrives (`host/web/glue.js:456`), `host/web/index.html` declares none, and an
app whose scheme state starts at `system` never sends the command until
someone clicks. CSS's initial `color-scheme` is `normal`, which does not opt
the page into either — so a first frame would resolve light under a dark
system, which is the exact bug this RFC exists to remove. The web host must
declare `color-scheme: light dark` before boot; that is what "no override is
the system's" means on that surface, and it matches the absent
`NSAppearance` on macOS and `.unspecified` on iOS.

Apple and Linux resolve at style application. On Apple it is not simply "a repaint", and r1 saying so understated it twice.

The appearance to resolve against is the **owning view's**
`effectiveAppearance`, not `NSAppearance.currentDrawing()`: `currentDrawing()`
names whatever is drawing at that instant, and colours are not all applied
inside a draw — a text field's `textColor` is assigned in
`NodeViewMac.applyStyle`, outside one.

And a repaint is not enough, because text colour is cached. `NodeText`
caches both the paragraph spec and the laid-out paragraph, and a `Run` carries
a concrete colour; inline text nodes are not in the native hierarchy at all.
So an appearance change must invalidate the colour-bearing text caches
(`invalidateText`) and re-apply the styles that were assigned outside a draw,
or the page background changes while the ink stays from the previous
appearance. `viewDidChangeEffectiveAppearance` is the hook, and UIKit's
`traitCollectionDidChange` is its counterpart.

### D3 — The appearance is one input to the host, and never a fact in the app

*Amended by LLP 1069.000 D1 (2026-09-27): the system's scheme is a fact, `exactViewport().prefersColorScheme` on every host, beneath any `setScheme`; a colour chosen by branching on it is refused (`lower-scheme-color`) in favour of a `light-dark()` pair, and the hidden `appearance` node below is deleted.*

`setScheme` stays exactly what it is: the app tells the host which appearance
to be — `light`, `dark`, or `system`, where `system` is the absence of an
override (LLP 1033 D6). Everything else follows from that one value.

Deleted, per app: every colour's dependence on the appearance. The theme
resource stops taking `scheme`/`systemScheme` for its **colours**, and each
one is a pair the app never resolves.

**One consumer survives, and r1 was wrong to claim otherwise.** `theme.icons`
is not a colour — it is a filename suffix (`""` / `"-dark"`) interpolated into
an image path, because a PNG has no `currentColor` to follow (LLP 1033 D6).
So the appearance fact and the node that reports it **stay**, doing one job
instead of twenty-one. That is a smaller claim than r1's and it is the true
one: this deletes the appearance from every colour, not from the app.

What would remove the last consumer is a painted `tint_color` — one artwork,
tinted by a row that could itself be a pair. The unused row has since been replaced by `caret_color`; **image tinting stays out**: LLP 1011 §6 puts it outside v1 explicitly. Moving it back on
costs a trade under `rules/RULES.md` §Scope and is not taken here; it is the
natural follow-up and the trigger is a second app wanting a tinted icon.

Not deleted either, therefore: `ExactDocuments.systemAppearance`,
`reportAppearance`, `watchAppearance`. What this does buy on the host side is
that iOS and the web never need their own copies **for colour** — the browser
resolves, and UIKit resolves — which is the half that was never written.

### D4 — A token that arrives as data is a pair too, so the theme record survives

A style value that is not a literal is parsed by the kernel at runtime:
`env(safe-area-inset-top)` is parsed from a *string*, and `check_style_value`
in `contract/lower` deliberately lets a non-literal expression through to
`set_dynamic`. So a data source may answer the string
`"light-dark(#ffffff, #17181b)"` and the row will hold the pair.

That matters more than it looks. It means LLP 1033's token record does not
have to change shape: `markdown-parse::theme` returns pair-strings instead of
single colours, and the readers' `provide`/`inject` threading, all 21 tokens
of it, is untouched. The apps lose only the appearance plumbing D3 deletes.

### D5 — It costs a wire format, and pre-1.0 that is the whole cost

A colour row gains a discriminant, so the style table's digest changes. The
plan envelope carries `kernelSchema` (an `exact.json` reads
`"kernelSchema":"b48b730a33a086af"` today) and the compatibility id is derived
over the app's inputs (LLP 1030 D3a), so every baked bundle stops matching and
is republished. It is more than a republish: the schema digest enters every
app's compatibility identity (`contract/cli/src/compat.rs`), and LLP 1030's
own inventory classifies a kernel change as one that needs a **new binary** —
a rebaked bundle cannot update an installed native decoder. So adoption is
rebuild-and-reinstall for every native host, Caltrain included, not only the
two readers that author pairs. `rules/DEFERRED.md` §Deliberately worse admits
exactly this —
"no backwards compatibility, at all, before 1.0" — and the generated decoder
follows from `kernel/tables/schema.json` because `kernel/build.rs` generates
it. It is a real consequence for anything already installed, and it is the
reason to do this while the only consumers are two readers nobody else runs.

### D6 — Motion is not affected, and the note that says otherwise is wrong

Stated because the author of this document told Charlie the opposite in
conversation and it should not survive into the corpus.

The wire vocabulary is `all`, `translate`, `scale`, `rotate`, `opacity`
(the schema's `_transitions` note), and `motion/src/property.rs` knows only the
four named ones. r1 read that as "no colour is animatable in v1". **On the web
that is false**, and the review caught it: `transition: all` lowers to literal
CSS `all` (`host/web/src/css.rs:227`), and CSS `all` means every animatable
property — colour included. So a colour under `transition: all` already
animates in a browser and jumps everywhere else.

That divergence is **pre-existing** — a literal colour change has always had
it — and scheme-dependent colours make it visible rather than create it. Two
consequences, both real:

- A native host resolving a pair changes a colour instantly. A browser under
  `transition: all` will *tween* between the two resolved colours when the
  scheme changes. Whichever is right, they should be the same, and today they
  are not.
- Settling it means deciding what `all` means across hosts — either the web
  lowers it to the four named properties and the deviation is declared, or the
  native evaluator grows colour. That is a motion decision with a motion
  fixture behind it, so it belongs to LLP 1002/1003 and is **not taken here**.

What survives from r1 is only the narrow part: nothing in the *native*
evaluator interpolates colour, so there is no ordering constraint between
resolution and `exact-motion` today. When colour animation does arrive it
acquires one obligation — resolve both endpoints before interpolating, because
a pair has no midpoint.

## 3. What this deliberately does not do

- **It does not replace tokens.** A palette is still worth naming in one
  place; `light-dark()` is what a token *holds*, not a reason to scatter
  literals through a view again. LLP 1033 D6's record stays (D4).
- **No `color-scheme` style row.** The appearance is app-wide and set through
  `setScheme`. Per-element `color-scheme` — a light card on a dark page — is
  CSS and is deliberately not taken: it needs the row, inheritance through the
  kernel tree, and a resolution context per node. The trigger is a real
  design that wants it. *Taken 2026-10-05, §8: Signal's story reply sheet,
  dark whatever the system's appearance, was that design.*
- **No other CSS colour functions.** Not `color-mix()`, not relative colour
  syntax, not `currentColor`. `currentColor` is the one worth naming: it is
  what would let a single icon follow a palette (LLP 1033 D6 ships two PNGs
  instead), and it is a different mechanism — inheritance, not a pair.
- **No image or asset variant selection.** A PNG per appearance stays the
  app's own business, named by a token as it is today.

## 4. Landing order

1. **The kernel.** Widen the colour codec in `kernel/tables/schema.json`;
   `build.rs` generates the rest. Parse `light-dark(a, b)` beside the existing
   colour parse, including from a dynamic string (D4). Nothing resolves here.
2. **The web host.** Emit the function. This should be a smaller diff than
   step 1, and it is the oracle the other two are held to.
3. **The Apple host.** Resolve at style application against the drawing
   appearance; re-apply on an appearance change.
4. **The Linux host.** Resolve against what the host is told; it has no system
   appearance of its own, so it takes the app's `setScheme` value.
5. **The readers.** Delete what D3 lists; turn the 21 tokens into pairs.
   `apps/markdown` and `apps/llp` are the whole consumer set.
6. **The agent.** `state` and `layout` should say which appearance a colour
   resolved under, or a pixel fixture cannot say which of two correct pictures
   it got (§5).

## 5. Open questions

1. **Does the agent need to report the resolved appearance, or force one?**
   Reporting is enough for a driver to explain a screenshot; forcing is what a
   pixel fixture needs to be deterministic across a machine whose OS setting
   nobody controls. Forcing is `setScheme` under a script, which already
   exists — so this may be a documentation question rather than a code one.
2. **Where does the Apple host read the appearance?** `NSAppearance.currentDrawing()`
   inside a draw is correct and per-view; a presenter-level cache is cheaper
   and wrong the day a view has its own appearance. Start correct.
3. **Does `tint_color` want the pair?** It is the image tint. If
   `currentColor` ever lands (§3) the two overlap, and it would be worth
   knowing which one an icon should use before both exist.
4. **Should the GPU canvas see resolved colours?** A surface takes Contract
   *expressions*, not style rows, so `light-dark()` does not reach it through
   D1 at all. Either surfaces stay unaware — the canvas's own rows resolve
   normally, its contents are the app's problem — or a resolved appearance
   becomes a surface input. The first is the smaller answer and probably right.

## 6. What this amends, at acceptance

- **LLP 1001 §1** — the style table's value vocabulary gains a deferred
  colour, and the declared deviation list gains nothing: this *removes* a
  deviation from CSS rather than adding one.
- **LLP 1033 D6** — its `systemScheme` slot, `appearance` node, and the macOS
  appearance reporting are superseded by D3 and should be struck from that
  document when this lands, with a line saying what replaced them.
- **LLP 1007 / 1008 / 1015** — each gains a sentence on how it resolves a
  colour, which is the whole of its host-side contract.

## 7. Landed, 2026-09-08

Built in the order §4 gives, by Claude at Charlie's direction, the same day
the RFC was written and reviewed.

**The kernel.** `ColorValue` is `Fixed(Color) | LightDark(Color, Color)` in
`kernel/src/style.rs`, beside `Dimension` and shaped like it. The wire codec
is tagged — a kind byte then one or two colours, as `dimension` is a kind
byte then an `f32` — and is a *new* codec (`color`), not the untagged
`color2`, which stays unused. The eight `rgba8` rows moved to it in
`kernel/tables/schema.json`; `build.rs` generated the rest, and the only
hand-written lines were the codec's read, write, and the `StyleValue`
accessor that parses `light-dark(a, b)` from text.

**Authoring cost nothing.** `check_style_value` in `contract/lower` already
passes a non-literal through to `set_dynamic`, and `set_dynamic` now parses
the function — so the compiler was not touched at all, and a *data source*
can answer the string `"light-dark(#ffffff, #17181b)"` and have the row hold
a pair. That is what D4 predicted and it held.

**The hosts.** The web emits the function and the browser resolves it
(`host/web/src/css.rs`), with `color-scheme: light dark` now declared in
`host/web/index.html` so a first frame follows the system — the fix for the
review's second finding. Apple carries the pair across the ABI as two
channel arrays and resolves against the **owning view's** `effectiveAppearance`,
with `viewDidChangeEffectiveAppearance` invalidating the text caches; inline
runs, which are detached from the view hierarchy and have no appearance of
their own, inherit their paragraph's. Linux resolves against the app's
`setScheme`, which that host previously had nothing to do with.

**The readers.** Twenty tokens are pairs and the theme resource takes no
arguments — one constant, never re-asked when the appearance changes. As D3
says, the appearance did not disappear from the apps: `iconSet` is still
derived from it, now in Contract rather than through the data source, because
a PNG cannot follow `currentColor`.

**Checked.** Two kernel tests (the parse, the resolve, the refusals, and a
row taking a pair dynamically as a dimension takes `env()`); one web test
asserting the pair reaches CSS as the function rather than being resolved
early — the parity claim of D2, which no screenshot can make. All five checks
pass; the fifteen Swift host tests pass; Caltrain's smoke is unchanged but
for its known cover-viewport failure. Driven on macOS: the app's `theme.page`
reads `light-dark(#ffffff, #17181b)` in `state` while the window draws dark,
which is the whole design in one line — the app holds the pair, the host
resolves it.

**Owed.** The web is built and renders, but a browser under a *dark*
preference has not been driven; headless Chrome prefers light and the agent
has no way to say otherwise. iOS is unbuilt against this. Both are §5's first
open question in practice, and neither blocks the two readers.

## 8. Amended 2026-10-05: per-subtree `color-scheme`

*Approved by Charlie 2026-10-05 ("Ok sounds good, go for it").* The trigger
§3 asked for arrived: Signal's story reply sheet is dark whatever the
system's appearance (`overrideUserInterfaceStyle = .dark`), and its
reactions and field are glass. Without a scheme per subtree a clone could
only paint fixed dark fills, because exact2's glass, platform colours and
`light-dark()` all follow the app's appearance.

**The row.** `color-scheme`, CSS's property, inherited as CSS's is, with
initial `normal`: the surrounding scheme (the app's, or an ancestor's). An
author writes `light` or `dark` (`color-scheme="dark"` on any box, text or
control). `light-dark()` colours, platform colours (LLP 1095) and native
materials on that node and below resolve in that scheme, and a nested value
wins below it. Refused by name (`lower-attr-value`): CSS's `normal`, because
in CSS it means the page's schemes, not the parent's, so an explicit one under
a `dark` subtree would differ between the browser and the native hosts; and
`light dark` and `only`, which ask a browser to choose. A bound value that
evaluates to `normal` unsets the row, as `unset` does, so it follows the
surrounding scheme on every host (the JS target removes the property).
Refused by position (`lower-attr-tag`): an inline `text` run, which paints
in its paragraph's scheme on every host, and SVG elements (an `svg` root takes
it; its shapes resolve with it). Leaving the attribute off follows the
surrounding scheme.

**The hosts.** Each maps it to its own per-view appearance, so nothing is
re-implemented:
- **iOS:** a node view with the row gets `overrideUserInterfaceStyle`; every
  view below the node that sets it carries the computed row (the Apple host
  sends it as it sends `dynamic-range-limit`), so a popover or dialog lifted
  out of its ancestor keeps it, and UIKit's trait inheritance covers the
  rest. Each node view re-resolves through its `UITraitUserInterfaceStyle`
  registration (LLP 1095 D5); the trait update is taken at once, so the same
  style pass reads the new scheme.
- **macOS:** the same, with `appearance` (`darkAqua` / `aqua`, `nil` when
  unset) and `viewDidChangeEffectiveAppearance` (D2).
- **The web:** `color-scheme: dark|light` on the element (`inherit` for a
  cleared row), and the browser resolves `light-dark()` under it.
- **Linux:** a node that sets the row paints, with its subtree, in that
  scheme; the painter carries a node's appearance to its children, as before
  for a view's own report.
- **Paint motion** (LLP 1062): a transition's endpoints resolve in the node's
  subtree's `color-scheme`, else a host's report for its view (D4 there),
  else the session's, so a scheme change moves its colours as an appearance
  change does; the view's report that follows on Apple confirms rather than
  corrects, and snaps nothing. On Apple a view's report is also what puts a
  keyframe animation's `light-dark()` colours in its appearance; reports made
  before the session's first scheme are made after it, and a new runtime
  hears them again.

The agent's `prefer prefers-color-scheme` still sets the system's scheme
beneath any override, and `exactViewport().prefersColorScheme` still reports
the system's (LLP 1069.000 D1): a subtree's scheme is paint, never a fact the
app reads.

**Not taken.** A scheme for images or assets (an app still names a PNG per
appearance); `color-scheme` on the root (an app's scheme is `setScheme`'s);
CSS's `only` keyword. **Keyframe animations of `light-dark()` colours** in a
`color-scheme` subtree follow a view's reports (LLP 1062 D4), not the commit,
and only its first: on Apple a view's first report that differs from the
session recolours what it plays (boot's correction); after that a playing
animation keeps the half it started with when the subtree's scheme changes,
including one that starts in the commit that changes it; and a report made
outside a batch (UIKit's trait walk after the system's appearance changes) is
said with the next batch. A subtree fixed to the scheme the session leaves is
reported only when its views next change appearance. Transitions have none of
this: they resolve by the commit. On Linux: a playing keyframe animation of a
`light-dark()` colour inside an overridden subtree keeps resolving in the
app's scheme (Linux reports no view appearance to the engine); a content
region (LLP 1093) resolves its runs in the app's scheme; and a frame with a
`color-scheme` subtree mounted repaints in full rather than by flow damage,
the rule the painter already has for any node painting in an appearance of
its own.
