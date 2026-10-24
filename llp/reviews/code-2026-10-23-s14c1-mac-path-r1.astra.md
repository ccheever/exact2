# Code review: stage 14 C1: macOS draws a template-path icon as its path, round 1 of 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `dc5cbbb17d`.
- **Method:** one brief (sha256 `b46d4655c492a5124effb75bb15b3a9a88ec43f13f1c2700558b8264b261cb1c`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

1. **MAJOR — Translucent tint is applied repeatedly at path overlaps.** [IconShapeMac.swift:48](host/apple/Sources/ExactKit/Mac/IconShapeMac.swift:48) assigns the tint’s alpha to every shape independently. With two overlapping paths and `color: rgba(0,0,0,0.5)`, the overlap becomes 75% opaque while the rest remains 50%. An offscreen Core Animation probe confirmed this. Previously, the template image combined the opaque paths before applying tint; ExactSnapshot still does that.

   **Fix:** fill the paths with opaque tint RGB and apply tint alpha once to their compositing group, with implicit actions disabled. Alternatively, use their combined mask for one tinted fill. Add an overlapping-path test with translucent tint; the current reference and probe use opaque black.

2. **MINOR — Paths can escape the placed em square.** [IconShapeMac.swift:83](host/apple/Sources/ExactKit/Mac/IconShapeMac.swift:83) computes the placed square but uses it only for the transform. The enclosing clip covers the entire content box. For a 48×24 content box, `object-fit: contain`, and viewBox `0 0 24 24`, a path extending from x=-12 to x=36 now paints across all 48 points. Previously, the 24×24 template image clipped that geometry before being centered, leaving the side gutters empty.

   **Fix:** also clip the shape group to the rectangle returned by `IconPath.placed`, retaining the existing content and rounded-corner clipping. Add a rectangular-box fixture whose path extends beyond its image bounds.

The reference is independent of `IconPath`, and the probe does create a real node through `Presenter.apply` and `updateSymbol`. Full XCTest execution was unavailable: this read-only checkout has no build artifacts. No files changed.

Verdict: changes requested — 1 MAJOR, 1 MINOR.
