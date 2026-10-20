# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and round 1.
- **Method:** one brief (sha256 `b3f403aa24c71fe70d1c89bccab32eb4928d76d7296e6a2857c78fb50dc29fc7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the first real crossing swallowed; a pooled row keeping a hold).
- **Disposition:** fixed in round 2's commit.

---

The round-1 fixes hold on the paths they name. Two of them still fail in cases the new code can actually reach.

1. **Blocking.** A later entry is ignored while `overKnown` is still false. `HeldState.swift:121` and `HeldState.swift:184`.

`hold` on a node with no look recognizer sets `overKnown = false` and `over = false`, then attaches the recognizer. UIKit does not deliver `.began` until the pointer enters, so `overKnown` stays false. The entry hits this and returns without `letGo`:

```118:122:host/apple/Sources/ExactKit/HeldState.swift
    static func lookHovered(_ node: NodeView, _ over: Bool) {
        guard !swapping, state == "hover", let held = view, held === node else { return }
        // Not known (a recognizer just put on, or cancelled): this tells it.
        if !overKnown { self.over = over; overKnown = true; return }
```

Hold hover on a plain node while the pointer is elsewhere, then move onto it. On the Mac that lets the hold go. Here the hold stays (`over` becomes true). The same return swallows the first `.began` after `.cancelled` when `over` was false, so a pointer that arrives after a cancel does not release either. `letGo`, `retire`, and `swap` do set `overKnown = true`; the false value is the one `hold` and `lookUnknown` leave in place until that first callback.

After `lookHover(true)`, if the recognizer is not already `.began`, set `over = false` and `overKnown = true` before `hold` returns. A synchronous `.began` during `addGestureRecognizer` still runs `lookHovered` while `overKnown` is false and records "already over" without releasing. Any later `.began` or `.ended` is a real crossing. On cancel, release when the next sample differs from the last `over` (a restart that is still over does not). `testIOSHoverLookAndHold` currently asserts the opposite (`HeldStateTests.swift:775` and `:765`): a `.began` after `hold` returns, and a `.began` after cancel while `over` was false, both keep the hold. Those two expectations are the bug.

2. **Blocking.** A list row is pooled while a hover hold's recognizer is still on a descendant, and the hold never lets go. `NodePoolIOS.swift:348` and `NodePoolIOS.swift:293`.

`lookAtRest` treats every `.possible` look recognizer as idle. A hold on a node with no `:hover` look is exactly that recognizer. Destroys run root-first (`PresenterIOS.swift:948` calls `holdGone` only on that id, then `pool.retire`). `holdGone` on the row does not match a hold on a child. `park` still sees the child's recognizer as at rest, and `release` recycles with `recycle()`, which does not call `holdGone`. The child's later destroy finds it already out of `views`, so `holdGone` does not run there either. The same hole is the inner-card `park` from the enclosing row (`NodePoolIOS.swift:302`).

Hold hover on a label inside a list row (no `:hover` look), with the pointer not on it, then scroll the row off or replace it. The channel never gets `gone`. `HeldState.view` still points at the parked view, so a pointer move on screen does not cross it. The recognizer stays on the pooled view until that shape is reused (`rebind` → `dropPressPaint` → `holdGone`, and `rebind` has already overwritten `id`) or evicted. A hold on the destroy op's own view is fine: `holdGone` runs first and `letGo` strips a hold-only recognizer before `recyclable`.

Call `holdGone()` at the start of `recycle()` so every pooled view lets go before it is reused. `letGo` then drops a hold-only recognizer and leaves a real `:hover` one. Also make `lookAtRest` return false while `HeldState.view === v && HeldState.state == "hover"`, so a `.began` hold is not the only thing that keeps the row out of the pool.

Checked, not findings: `letGo` / `retire` / `swap` do remove the hold's recognizer (`HeldState.swift:71` runs only after `view` and `state` are cleared, so `lookHover`'s "keep it while held" override does not apply). A style change during the hold keeps the recognizer (`LookHoverIOS.swift:21`) and `letGo` removes it if `lookBase` no longer has a hover look. `removeTarget` before `removeGestureRecognizer` stops removal from re-entering `presented`. Mac `hold` / `hovered` is behind `#if os(macOS)` and does not read `overKnown`. tvOS still rejects hover in `hold`; the deleted test could not run because `HeldStateTests.swift` is `#if os(iOS) || os(macOS)`.
