# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change and its first two rounds.
- **Method:** one brief (sha256 `ce2ad40156435625a08a37068b42b72edad60b0ae28c53626679f946bca478cf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (a P1: a nearer bare style's clear hid a further one's fill; a P2: a dynamic heading on headline dropped its weight on the JS target).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

I'll review the D12.2 weight and leading change read-only, starting with the review criteria and the three commits from HEAD~2 through HEAD.The review stays read-only on HEAD~2 through HEAD. I'll pull the three commits and the files each round touched, then check measured values against what hosts paint and what a page inherits.Round 2 narrowed which written rows beat a bare style and made the document writer emit those rows. I'll read the resolution path and the hosts that paint it.The bare-style clear is a real measure-versus-page split, so this is not done. One other hole is on the JS target. The invalidation added for a size or `fontShorthand` change does mark descendants, including a child with its own size.

**[P1] A nearer bare style that clears a row hides a further bare style's fill — `kernel/src/arena/text_style.rs:120`**

`bare_text_style` returns the nearest bare `font-size` and that node's `written` mask, and stops. `copy_inherited` records the same pair (`kernel/src/arena/defaults.rs:313`) and never walks to a further bare style. A clear sets `written` and drops the row from the mask, so the filter at `computed_source` (`defaults.rs:239`) and `text_style_source` (`defaults.rs:276`) rejects the row and falls through to the CSS initial.

```
column font-size="-exact-headline"
  text font-size="-exact-body" font-weight="inherit" "Subtitle"
```

On the web column the kernel measures weight 400. The live host and the document writer still declare `font-weight:600` on the column (`element.rs` `with_style_rows` / `style_given`) and `font-weight:unset` on the text, so the page inherits 600. The same 400 is what a `font-size=12` grandchild measures, and what that text's runs measure. A run of the same shape inside the headline paragraph stays 600, because the parent's computed rows are already in hand. A shorthand or a weight stored on an ancestor is fine. Only a further bare fill is skipped. Round 2 fixed a clear under a bare style that the child does not itself name.

**[P2] A dynamic heading on headline drops its weight on the JS page — `host/web-js/src/rows.rs:319`**

A level that chooses a title or a headline lowers `font-weight` to a ternary of `700` and `none` (`contract/lower/src/headings.rs:93`). `none` clears the row with `written` false, so the kernel's shorthand still supplies headline's weight (600 on the web column, 700 on the Mac). The JS target treats any `font-weight` binding as the node writing the row, skips `TEXT_STYLE_ROWS`, then `css` in `rt.js` removes the property when the value is null. The class has no fallback weight, because the size is dynamic and was not in the static kernel.

```
text role="heading" aria-level=(big ? 1 : 5) "Section"
```

With `big` false the kernel measures 600 and the JS page computes 400. A static headline is fine: no weight binding is emitted, and the class carries the style's weight.
