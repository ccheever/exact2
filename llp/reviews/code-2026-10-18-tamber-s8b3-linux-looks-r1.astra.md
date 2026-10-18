# Code review: Linux's absolute press opacity and looks (LLP 1121 stage 8 B3), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** `23fd7820f`, the Linux host and Android through it.
- **Method:** one brief (sha256 `51a6d08d7ab0d59233af23cf646235317791fc524c38a175d104d527d064d25e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (blocking: a look's opacity kept in the presentation cache, hover looks stale under a resting pointer, Android's lowered opacity overriding; should-fix: a look's currentcolor ignored).
- **Disposition:** in the round-1 commit, with tests (`looks_tests.rs`: a transition started under a hover leaves opacity 1 after it, failing without the fix; the agent's hover). 1: the cache starts from the committed style (`unpresented`). 2: `hover_at` refreshes the looks from its hit, a resting pointer's too. 3: a track the host paints over is not lowered (`press::paints_over`). 4: currentcolor is the node's colour.

---

**RIGHT WITH CHANGES**

1. **BLOCKING — Look opacity can become permanent base opacity.** [host/linux/src/host.rs:683](/tmp/rv-s8b3-r1/host/linux/src/host.rs:683)  
   `presented()` now includes look opacity, but `Host::present()` and `paint_over()` use it to initialize the presentation cache. Hover a previously unanimated node at opacity `0.8`, then start a scale transition: the cache captures `0.8`. Leaving hover restores that cached value instead of the authored opacity `1`. Initialize cached presentation from values without host feedback; apply looks and press only when reading for presentation. Add a hover → unrelated transition → leave regression test.

2. **BLOCKING — Hover looks remain on stale targets after scrolling or layout changes.** [host/linux/src/presenter/feedback.rs:80](/tmp/rv-s8b3-r1/host/linux/src/presenter/feedback.rs:80)  
   Painting retains every previously hovered ID that still exists. The existing `follow_pointer()` path updates hover handlers but never calls `refresh_looks()`. Consequently, scrolling beneath a stationary mouse leaves the old node highlighted and the newly covered node unhighlighted. A node becoming inert can also retain its look. Recompute the look path from current hit geometry after presentation, independently of hover handlers, and invalidate both departing and arriving kept rows.

3. **BLOCKING — Android’s lowered animations overwrite the new absolute press opacity.** [host/linux/src/press.rs:79](/tmp/rv-s8b3-r1/host/linux/src/press.rs:79)  
   The composed opacity reaches Android as the layer’s base value. For a node with a reader-executed opacity animation, [Layers.kt:317](/tmp/rv-s8b3-r1/host/android/canvas/src/main/java/dev/exact/canvas/Layers.kt:317) replaces that value with the animation sample. A fully held `:active opacity=0.5` therefore continues showing the animation’s opacity. Keep affected tracks sampled in Rust, or carry feedback parameters to the reader and compose them after sampling. Cover this with a Canvas-reader test; CPU painter tests cannot catch it.

4. **SHOULD-FIX — `currentcolor` look declarations are silently ignored.** [host/linux/src/paint/presented.rs:260](/tmp/rv-s8b3-r1/host/linux/src/paint/presented.rs:260)  
   Background and border colors represent `currentcolor` as `None` with their mask bit set. The new `as_ref()` checks treat that as an absent declaration, leaving the base or lower-priority look visible. Check the mask first, then resolve `None` against the node’s presented text color. Test both background and border overrides.

Tests could not start: the read-only sandbox denied creating the requested Cargo target directory. Findings are based on source inspection.
