# Review: LLP 1010 Scrolling v1 (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only`, `gpt-5.6-sol`, `model_reasoning_effort=xhigh`, workdir = a repository export
- **Method:** llp-review, one round at Charlie's request, 2026-08-29; mutually blind to the other family. Brief sha256 ed01727ee8dbe70083ca9b62040bc6c55ca9ed28ef6d00e82ccd614c75af513a; target sha256 d5a9e9ecade3babcbbabfc325ae06848f9349871bca4a3954aa312808c5ddd78.
- **Verdict:** **NOT READY** (session 01a04d02-7848-7cf3-bf2b-22e8eceb255e)

---

## Verbatim review

## Overall assessment

The host-owned offset model and current Apple chaining algorithm are mostly transcribed accurately. The spec is not ready because it overstates CSS parity, scrollable-overflow semantics, page sizing, and harness coverage, while omitting decisions needed for `List` windowing.

## Findings

1. **MATERIAL —** §1 says `scroll` and `hidden` mean “children do not grow” the node (`llp/1010-scrolling-v1.spec.md:36-38`). Taffy still derives an auto block height from its children regardless of overflow (`vendor/taffy/src/compute/block.rs:223-236`); §3 itself relies on this when explaining why Caltrain’s auto-height scroll node does not overflow (`llp/1010-scrolling-v1.spec.md:110-112`). Replace the claim with the actual sizing/overflow-propagation rule and state that scrolling requires a constrained used size.

2. **MATERIAL —** The effective-overflow rule is not CSS’s computed-value rule. §1 says unset x “follows” non-visible y (`llp/1010-scrolling-v1.spec.md:46-48`), and the kernel and Apple host literally copy y into x while never performing the symmetric computation (`kernel/src/style.rs:447-460`, `host/apple/src/style.rs:70-86`). CSS instead computes a visible counterpart to `auto`: for example, `overflow-y:hidden` with x unset makes x `auto`, not `hidden`, while non-visible x with visible y also changes y. Explicit `visible` is mishandled similarly. Implement the symmetric CSS computation—introducing or deliberately mapping effective `auto`—and test all authored/default combinations across kernel, web, and Apple.

3. **MATERIAL —** The root-width rule is only equivalent to CSS `width:auto` in the unconstrained case. §1 declares only margins and absolute positioning as deviations (`llp/1010-scrolling-v1.spec.md:49-57`), but `taffy_style` also silently changes the root’s authored/default `content-box` sizing to `border-box` (`kernel/src/style.rs:551-565`). Consequently, `min-width` and `max-width` constrain the border box rather than the CSS content box. Implement true block used-width sizing from the offer or declare and justify the min/max and box-sizing deviations; add constrained padded-root tests.

4. **MATERIAL —** `NodeRef.content` is not itself CSS `scrollWidth`/`scrollHeight`, and macOS page sizing ignores it where it matters. The kernel stores raw Taffy `content_size` (`kernel/src/layout.rs:263-273`); block content is only the maximum child contribution (`vendor/taffy/src/compute/block.rs:274-280`) and does not consistently include end padding, unlike Taffy’s explicit flex end-padding adjustment (`vendor/taffy/src/compute/flexbox.rs:2192-2195`). Apple must separately max it with the node’s box (`host/apple/src/host.rs:405-412`). Moreover, `Presenter.fitDocument` sizes the page only from root frames (`Presenter.swift:249-256`), so a fixed-height or absolute-child root with visible descendant overflow is unreachable although the web document scrolls to it. Define the published extent precisely, compute CSS-compatible scrollable overflow, and size the page from each root’s overflow extent. Cover empty, padded, fixed-height, deeply overflowing, and absolute-descendant roots.

5. **MATERIAL —** §3’s claim that the presenter acts on current effective overflow is false after a dynamic `scroll → visible` change (`llp/1010-scrolling-v1.spec.md:86-96`). `NodeView.applyStyle` creates an `NSScrollView` but never removes it (`Presenter.swift:125-150`); `container` therefore remains its clipping document view (`Presenter.swift:101-102`) even after both axes become visible. Contract style bindings can change at runtime (`contract/lower/src/lib.rs:520-527`, `runner/src/instance.rs:248-279`). Reconfigure or remove/reparent the wrapper when effective overflow changes and test transitions among `visible`, `hidden`, and `scroll`.

6. **MATERIAL —** §4 overclaims what the harness asserts (`llp/1010-scrolling-v1.spec.md:129-144`). `smoke.mjs` does not parse either direct-delivery report, does not assert the Caltrain page reaches its end, does not assert the fixture’s exact inner/page limits, and does not check the fixture root is 420 points wide (`host/apple/smoke.mjs:24-40`). It does assert hit-delivery exclusivity and that the page stays fixed while the inner node can scroll. The phase-less, manually hit-tested limitation is honestly declared. `FLAG:` the recorded values `2348`, `652`, `471`, and the claimed successful 171-test run cannot be verified from retained source output. Separate observations from assertions and add checks for every invariant claimed as held, plus targeted overflow/content tests.

7. **MATERIAL —** §5 does not provide enough seam for the next `List` implementation and overstates that “the kernel already knows a list’s content extent” (`llp/1010-scrolling-v1.spec.md:152-166`). That extent is derived only from materialized layout children (`kernel/src/layout.rs:258-289`); after windowing it cannot represent the logical list. The runner admits only press/change events (`runner/src/runner.rs:42-49`), while `scrollCommand` and `virtualized` are inert schema props (`kernel/tables/schema.json:55-56`). Specify the logical total-extent/spacer contract, window-origin compensation, offset/event delivery, and programmatic-scroll interaction, or explicitly state that the current seam does foreclose windowing without extension.

8. **MINOR —** §5 mixes absent features with built-but-unverified behavior. Web `overflow:hidden` lowering and macOS horizontal scrolling exist, which the text itself acknowledges (`llp/1010-scrolling-v1.spec.md:155-163`). Contract also already exposes the two-axis `overflow` shorthand (`contract/lower/src/tags.rs:213`), although §1 mentions only the missing `overflowX`/`overflowY` names. Split “not built” from “built but unverified” and document the existing shorthand.

9. **MINOR —** The focused spec conflicts with binding LLP 1008. LLP 1010 correctly describes direct deterministic consumption and loss of inner rubber-banding (`llp/1010-scrolling-v1.spec.md:103-118`), while LLP 1008 still describes an AppKit-first fallback and AppKit-owned inner behavior (`llp/1008-apple-host-v1.spec.md:141-149`). Mark that paragraph superseded or update LLP 1008.

## Verdict

NOT READY

---

## Disposition (orchestrator, 2026-08-29) — verdict bound to r1 (d5a9e9ecade3babcbbabfc325ae06848f9349871bca4a3954aa312808c5ddd78); r2 folds it and is unreviewed

1. MATERIAL (sizing claim wrong) — FOLDED at §1: overflow changes contribution and clipping, not the node's own auto size; a node scrolls only under a constrained used size.
2. MATERIAL (pairing asymmetric and copies `hidden`) — FIXED in `to_taffy` and `effective_overflow`: symmetric; a `visible` axis beside a non-visible one becomes `scroll` (CSS's `auto`); unit test extended.
3. MATERIAL (border-box switch changes min/max semantics) — DECLARED at §1.
4. MATERIAL (`content` not CSS scroll extent; page ignores it) — PARTLY FIXED: the host floors Taffy's content with the padded child extent and the box; the page extent is declared as the roots' frames with descendants past them outside it (§3, §5).
5. MATERIAL (wrapper never removed on `scroll → visible`) — FIXED in `applyStyle`: children come back out and the scroll view is removed.
6. MATERIAL (harness overclaim) — FOLDED as grok 1.
7. MATERIAL (`List` seam overstated) — FOLDED at §5: the current seam does not support windowing without extension; the three missing pieces named.
8. MINOR (§5 mix; the `overflow` shorthand exists) — FOLDED at §1 and §5.
9. MINOR (LLP 1008 §5 contradicts) — FIXED: 1008 §5 now points to 1010.
