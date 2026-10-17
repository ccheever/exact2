# Code review: a selection's outlines and scrolling (LLP 1122 T2b follow-up), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `04e114daa`.
- **Method:** one brief (sha256 `b18f03b820a9f9992028b45f2130127ce06ec70a97c42d8965e19cfbc55d73af`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken by a different fix. The page no longer guesses what is in view. The outline the server answers a selection made on this page with scrolls nothing, because the copy clicked is under the pointer (`inspecting.clicked`, set when the page reports its selection). Any other outline, the tool's Outline included, scrolls its first copy in as D4 says. That removes the viewport, clip, banner and empty-box cases. The test counts the page's `scrollIntoView` calls: none for the selection's outline (the old line made one), one for the tool's Outline.

---

- **Medium — [host/web/dev.js:632](/tmp/rv-dt-scroll/host/web/dev.js:632): Window overlap does not establish visibility.** A mounted virtual-list overscan row at `y=450` passes this check in a 900px window even when its scroll container clips at `y=400`. Likewise, a copy completely covered by the inspect banner passes. Either can suppress scrolling while no copy is actually visible. **Fix:** intersect with ancestor clipping regions on both axes, account for banner occlusion, and make scrolling reveal a copy in that unobscured region.

- **Low — [host/web/dev.js:632](/tmp/rv-dt-scroll/host/web/dev.js:632): Empty boxes count as visible.** A zero-size copy at `(50,50)` satisfies all four comparisons. With the first, nonempty copy offscreen, this prevents revealing it. **Fix:** require positive-area geometry and intersection. A `display:none` element’s all-zero rectangle correctly fails this predicate, although choosing it as `found[0]` remains a pre-existing fallback problem; select a rendered candidate.

- **Medium — [host/web/dev.js:633](/tmp/rv-dt-scroll/host/web/dev.js:633): The selection exception also changes explicit Outline requests.** Automatic selection outlines and the tool’s Outline button produce indistinguishable frames. If the first use is offscreen but a later use is visible, clicking Outline now leaves the first use offscreen, contrary to [D4’s first-copy behavior](/tmp/rv-dt-scroll/llp/1122-design-tool.rfc.md:321). **Fix:** carry a scroll policy distinguishing automatic selection outlines from explicit requests, preserving the latter’s behavior.

- **Medium — [scripts/tamber-server.test.mjs:477](/tmp/rv-dt-scroll/scripts/tamber-server.test.mjs:477): The assertion does not prove the regression is fixed.** It selects the single, already-visible “Swap pins” text, for which the old `scrollIntoView({block:"nearest"})` already leaves scrolling unchanged. It also captures the baseline after selection, when outlining may already have scrolled, and observes only window `scrollY`. **Fix:** arrange two mounted copies with the first offscreen; capture both axes of the actual scroll containers before clicking the later visible copy; await that outline operation and assert unchanged offsets. Add the complementary case where neither copy is visible and scrolling must occur.

Partial overlap intentionally suppresses scrolling, and horizontal viewport overlap is checked. Unmounted virtual rows are excluded. Tests were not run under the read-only restriction.

Verdict: Request changes—the visibility check is incomplete, and the added test would not catch the original bug.