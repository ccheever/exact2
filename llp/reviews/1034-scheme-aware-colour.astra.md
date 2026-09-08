# LLP 1034 review — gpt-6-astra, high

**Document:** `llp/1034-scheme-aware-colour.rfc.md` (as written 2026-09-08)
**Reviewer:** gpt-6-astra at `model_reasoning_effort=high`, via `codex exec`, read-only sandbox
**Requested by:** Charlie Cheever, 2026-09-08 — "look for major problems, don't try to correct everything"
**Orchestrated by:** Claude (Opus 5), which authored the document and is therefore not an independent reviewer (LLP 0005; `llp-review` invariant)

Verbatim. Four of its findings were independently checked against the
source before the document was revised; see the disposition note at the
end, which is this session's and not the reviewer's.

---

1. **Overall**

The direction is sound: deferred colour values fit the existing style model, and letting the browser resolve `light-dark()` follows the repository’s rules. I would revise the RFC before accepting it: the promised deletion of appearance plumbing is incomplete, and several host assumptions are wrong.

2. **Major problems**

- **Serious — D3’s deletion breaks the existing icon selection.**  
  D4 and §4 say all 21 tokens become pairs, but `theme.icons` is a filename suffix, not a colour: see [theme.rs:70](/Users/ccheever/projects/exact2/apps/markdown/parse/src/theme.rs:70). The readers interpolate it into image paths in [app.contract:275](/Users/ccheever/projects/exact2/apps/markdown/app.contract:275). Making `theme()` constant freezes that suffix; making it a colour-pair string produces an invalid asset path. Meanwhile §3 explicitly excludes asset selection and says it remains the app’s responsibility—even though D3 removes the information needed to perform it.

  **Resolution:** decide how these existing icons remain readable in both appearances before deleting their selector. Either retain the necessary appearance input or explicitly change the icon treatment. The current “everything stays except plumbing” promise cannot hold.

- **Serious — D2 misses first-load system appearance on the web.**  
  [glue.js:456](/Users/ccheever/projects/exact2/host/web/glue.js:456) sets `color-scheme` only when a `setScheme` command arrives. The reader initializes `scheme = "system"`, but calls that command only from `chooseScheme`: [app.contract:79](/Users/ccheever/projects/exact2/apps/markdown/app.contract:79). [index.html](/Users/ccheever/projects/exact2/host/web/index.html) supplies no initial scheme declaration. CSS’s initial value is `normal`, which does not itself opt the page into light/dark support. Consequently, emitting the function alone does not deliver the promised system-following first frame. [CSS specification](https://www.w3.org/TR/css-color-adjust-1/#color-scheme-prop).

  **Resolution:** specify the initial host scheme and establish it before first pixel, including the web’s `light dark` opt-in. Verify a fresh launch under a dark preference before any switcher interaction.

- **Serious — D2’s Apple repaint mechanism omits cached text and uses two different resolution contexts.**  
  Backgrounds resolve during drawing, but input colours are assigned during [NodeViewMac.applyStyle](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:669), outside drawing. `NSAppearance.currentDrawing()` identifies the active drawing appearance; it is not automatically the target view’s appearance during arbitrary style application. [Apple documentation](https://developer.apple.com/documentation/appkit/nsappearance/currentdrawing%28%29).

  More significantly, [NodeText.swift:21](/Users/ccheever/projects/exact2/host/apple/Sources/ExactKit/NodeText.swift:21) caches both paragraph specifications and rendered paragraphs containing concrete run colours. Inline text nodes are detached from the native hierarchy. Repainting or merely reapplying styles does not invalidate those caches, so the page background can change while paragraph ink remains from the previous appearance.

  **Resolution:** specify how unresolved pairs cross the Rust/Swift boundary, resolve against the owning view or paragraph’s appearance, and invalidate colour-bearing paragraph caches on appearance changes. Include the UIKit trait-change path. Avoiding kernel relayout remains plausible; “just repaint” understates the work.

- **Serious — D6’s motion exclusion is false on the web.**  
  [css.rs:225](/Users/ccheever/projects/exact2/host/web/src/css.rs:225) emits `TransitionProperty::All` as literal CSS `all`. That includes animatable colour properties; it is not restricted to Exact’s four native motion properties. [CSS Transitions specification](https://www.w3.org/TR/css-transitions-1/#transition-property-property). The native evaluator’s vocabulary is limited to those four in [property.rs:15](/Users/ccheever/projects/exact2/motion/src/property.rs:15).

  Thus colours can transition on the web while native colours change immediately. This mismatch already exists for literal colour changes; scheme-dependent colours make it directly relevant here.

  **Resolution:** settle the meaning of `all` across hosts. If v1 restricts it to four properties, enforce that restriction in web lowering and declare the CSS deviation. D6 cannot dismiss the issue by citing the wire vocabulary.

- **Worth knowing — D5 understates the deployment scope.**  
  Republishing bundles cannot update an installed native decoder. The schema digest enters every app’s compatibility identity in [compat.rs:368](/Users/ccheever/projects/exact2/contract/cli/src/compat.rs:368), and [LLP 1030’s inventory](/Users/ccheever/projects/exact2/llp/1030-delivery-unified.rfc.md:95) classifies kernel changes as requiring a new binary. The affected consumers include Caltrain and other kernel users, even if only the readers author pairs.

  **Resolution:** state that adoption requires rebuilding and reinstalling native hosts, alongside rebaking bundles. No compatibility shim is needed or warranted.

3. **Claims checked and found false**

D6’s “No colour is animatable in v1” is false across hosts. D4/§4’s conversion of all 21 tokens is false for `icons`. D2’s existing web plumbing is insufficient at startup.

Also, §4’s “`build.rs` generates the rest” overstates existing support: `color2` is a fixed `[Color; 2]`, and its byte codec is handwritten in [codec.rs:178](/Users/ccheever/projects/exact2/kernel/src/wire/codec.rs:178). It does not already provide a tagged literal-or-pair value.

These findings come from source inspection and platform specifications; I did not run a modified implementation.

4. **Recommended next step**

Revise this RFC in place to settle icon selection, initial appearance, native text invalidation, and `all` semantics. Then prove one reader through the existing verification paths: fresh dark-system launch, appearance changes without app-data changes, inline text and inputs, and a colour under `transition: all`. No additional governance apparatus is needed.

---

## Disposition (Claude, 2026-09-08 — not the reviewer's words)

Checked against source before revising:

| Finding | Checked | Verdict |
| --- | --- | --- |
| `transition: all` lowers to literal CSS `all` | `host/web/src/css.rs:227` | **Confirmed.** Colours can transition on the web. D6 was wrong. |
| `color2` is an untagged `[Color; 2]` | `kernel/src/wire/codec.rs:178` | **Confirmed.** Not a tagged literal-or-pair; §4 overstated what exists. |
| The web sets `color-scheme` only on `setScheme` | `host/web/glue.js:456`, `host/web/index.html` | **Confirmed.** No initial declaration; a first frame cannot follow the system. |
| `theme.icons` is a filename suffix, not a colour | `apps/markdown/parse/src/theme.rs`, both `app.contract`s | **Confirmed.** D3 would delete what selects it. |
| `tint_color` could tint one artwork instead | `llp/1011-image-v1.spec.md` §6 | Row exists, **nothing paints it** — explicitly not in v1. Not a free answer. |

Taken into the document: D1a (tagged codec, not `color2`), D2a (initial
scheme before first pixel), D2b (Apple's resolution context and the text
caches), D3 (the icon consumer survives — the deletion claim was too broad),
D5 (native binaries, not just rebakes), D6 (corrected and inverted).

Not taken, deliberately: unifying `transition: all` across hosts. It is a
pre-existing divergence that scheme-dependent colours make visible rather
than create, and fixing it is a motion change with its own fixture. Recorded
as a consequence in D6 and left for LLP 1002/1003.
