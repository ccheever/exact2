# Code review: a segmented tablist is measured only when what sizes it changes (3adba3171), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `3adba3171`, plain output.
- **Method:** one brief (sha256 `629bc35622c922396527c1081f2eab719d887984dff8a927b6fcab63a9654625`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken with astra's 2. The legibility weight is folded into the key.

---

LAND WITH CHANGES

No prior `code-2026-10-05-segment-measure-cache*` review exists. The idle-tick hole is still closed: round 2 was `segments.sync()` not running, and round 3 routes that through `requestProjectionSync` (`PresenterIOS.swift:36`). This change still runs that sync.

1. **MINOR** — Nothing fails if the cache ignores a title, image size, font, or text-size change. `PresenceIOSTests.swift:110` checks the first report against `intrinsicContentSize`, `:116-123` that a second `sync` and a resize do not publish again, and `:127-129` that border-box padding is added on top. A cache that never invalidates still passes. The tab-bar test (`PresenceIOSTests.swift:80-83`) does pin the other branch: a narrower box must republish width.
   Fix: after the first report, change a segment title and assert the next report equals `control.intrinsicContentSize`. Separately set `traitOverrides.preferredContentSizeCategory`, call `updateTraitsIfNeeded()` and `sync()`, and assert the same.

Checked, not defects:

- **Height is the only visible output.** Layout uses the intrinsic height as `min-height` and drops the width (`kernel/src/style.rs:1422-1431`). LLP 1059 measured that height on the iOS 27.1 simulator does not follow Dynamic Type or bold text. A stale width does not move the box.
- **Key vs inputs.** Titles and image point-size are read after `content()` (`SegmentsIOS.swift:206`, `:365-372`). A late decode is drawn into a bitmap of `icon.bounds` (`:145-156`); the same bounds keep the same intrinsic size, and a bounds change changes `image.size`, which is in the key. This host never sets `setWidth`, `apportionsSegmentWidthsByContent`, `contentOffset`, title attributes, or `UISegmentedControl.appearance`. Normal and selected fonts are still sampled (`:205`). The control is a subview of the tablist (`:351`), not a bar, so compact bar metrics do not apply. tvOS uses the same path; idiom does not change at runtime, and there is no focus-sized branch.
- **Stale publish.** `clearSize` drops `naturals` before `sizes` (`:197-201`). `restore` / `removeBar` go through it. Padding and border are applied after the cache (`:225-228`). A hit still goes through `sizes` (`:236`), so an unchanged size is not republished.
- **Tab bar.** The `sizeThatFits` branch (`:230-234`) is untouched. `clearSize`'s extra map removal is unused there.
- **Idle tick.** Sync still updates enabled, selection, frames, and membership (`:355-372`) on every batch and on the out-of-batch projection sync. The old per-batch `intrinsicContentSize` only repaired size changes the key now tracks. It did not repair the carried-row disable or frame-replay holes.
- **Legibility weight** is the one input `TitleSegments` keys (`NavigationBarIOS.swift:282`) and this cache does not (`SegmentsIOS.swift:207`). Bold text can change width. Height does not, and width is not read, so the box matches the old code. Fold `traitCollection.legibilityWeight` into the key if the stored width should match `intrinsicContentSize`.
- **Quality.** `SegmentsIOS.swift` is 408 lines. The new comment matches the file. No dead path. `rules/RULES.md` budgets are unaffected.
