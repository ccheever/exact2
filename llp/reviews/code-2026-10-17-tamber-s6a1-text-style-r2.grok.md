# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and its first round.
- **Method:** one brief (sha256 `2f902c7c94682859db8bff2b0a777d629e96e6c2fe6b725678160ebdba0d7085`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (P1s: the document and the adopted first paint omitted the rows, `inherit` under a bare style measured 400).
- **Disposition:** in the second-round commit.

---

I'll review the round-1 fixes read-only: first the review brief and the HEAD~1..HEAD diff, then each listed path for measurement, paint, and inheritance disagreements.Round 1 is `07ab1f91` on top of the D12.2 change. I'll read that fix and the surrounding text-style paths for measurement, paint, and cache mismatches.Two blocking holes. The other round-1 fixes hold.

**[P1] The document and the adopted first paint omit a text style's weight and leading — `host/web/src/document.rs:570`**

`css_style_named` (`host/web/src/element.rs:609`) writes those rows, and `view_css` uses it. The document walk still calls `css_style_of`, and the first batch keeps that CSS (`host/web/src/host.rs:1371`, fed from `host/web/src/page.rs:534`). `css_style_named` is imported in `host.rs:38` and never called.

`column font-weight=700` / `text font="-exact-headline"` measures 600 on the web column (the shorthand resets the 700). The served page has no `font-weight` on that text, so it inherits 700. The same for `line-height: 2` around `font="-exact-body"`: the kernel resets to `normal`, the page keeps 2. A later `update` recomputes `view_css` and the weight pops in; until then the mirror keeps the document CSS. `Host::boot` never goes through this cache, so the new tests do not see it.

**[P1] `font-weight="inherit"` under a bare text style measures as CSS's 400 — `kernel/src/arena/defaults.rs:314`**

`written` is unioned from the node up through the bare style, and the bare fill then drops those rows (`defaults.rs:332`). The same snapshot is in `bare_text_style` (`kernel/src/arena/text_style.rs:108`). A clear on the style node itself is right (the new test: nothing above, 400). A clear on a descendant suppresses the ancestor's bare weight, which is the value `inherit` is asking for.

`column font-size="-exact-headline"` / `text font-weight="inherit"` measures 400. The live host declares `font-weight: 600` on the column and `font-weight: unset` on the text, so the page inherits 600. A shorthand ancestor still wins, because that row is a start and is copied on the walk. An inline run under a bare paragraph can still measure 600 off the parent's already-filled rows while `computed` on that run is 400.

The run path, the shorthand-before-control order, the size/`fontShorthand` invalidation, the button title's weight bit, and a bound shorthand's `TEXT_STYLE_ROWS` map all match the kernel for the cases they cover. The JS target still leaving a bare `font-size` to CSS is the split the as-built row describes.
