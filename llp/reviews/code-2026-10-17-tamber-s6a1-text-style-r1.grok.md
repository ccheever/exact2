# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `759d1bc8be06c69f6cd272d99fec86bf2de4a0395c00b5df0574c6c3f9497317`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** changes requested (P1s: runs took no style, a bare style stopped at the next size, a control start beat the shorthand, a bound style moved only the size on the JS target; a P2 on a written clear).
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only against LLP 1115.000 D12.2: text-style weight and leading precedence, every path that reads those rows, and the hosts that used to read the removed `font` shorthand fields.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is read-only. I'll confirm the precedence paths and the line citations against HEAD before writing findings.No P0. Block text under a non-text parent matches the ruling and `contract/cli/tests/text_style.rs`. Four P1s sit off that path.

**[P1] Inline runs never take a named style's weight or leading — `kernel/src/arena.rs:679`**

`text_runs` builds the paragraph root with `copy_inherited` (`kernel/src/arena.rs:594`), then each nested `text` only overlays its stored style (`RunRows::child`, `kernel/src/text.rs:1007`). The shorthand no longer stores `font-weight` or `line-height`; those exist only as `text_style_start` / the bare fill (`kernel/src/arena/text_style.rs:56`, `kernel/src/arena/defaults.rs:318`). A paragraph `font-weight=700 line-height=2` containing `text font="-exact-body"` measures at 700 and line-height 2, while `computed` is the body style (400, and the Mac leading). A paragraph that sets neither, containing `text font-size="-exact-headline"`, measures at 400 and `normal` while `computed` is headline (600, or 700 on the Mac). Layout uses these runs (`kernel/src/layout.rs:1189`). The JS target emits the longhands on the naming node (`host/web/src/template.rs:65`), so the browser matches `computed` and the native measurer does not. The ruling's "stays bold" span still works, because the parent run already has the weight and the child does not overlay it. The new test only puts those texts under `column`, so each is its own paragraph root.

**[P1] A bare style's weight inherits in CSS and stops at the next font-size in the kernel — `kernel/src/arena/text_style.rs:69`**

`bare_text_style` is the nearest stored `font-size`. `copy_inherited` captures it once (`kernel/src/arena/defaults.rs:309`) and will not walk past a later size. A column `font-size="-exact-headline"` with a child `text font-size="16px"` therefore measures at 400 and `normal`. `style_given` still emits `font-weight` and `line-height` on the column (`host/web/src/template.rs:59`), and the child inherits them. On the Mac that is 700 and `1.2308` in the page, and regular in kernel layout. A child that does not set its own size, or that names its own bare style, agrees on both sides. A shorthand parent also agrees, because `text_style_start` is applied on the way up for every descendant.

**[P1] A control start now beats the shorthand's weight and leading — `kernel/src/arena/defaults.rs:161`**

`style_starts` tries `control_text_start` before `text_style_start`, and the first start that has the row wins (`kernel/src/arena/defaults.rs:318`). Native `starting_style` masks `font-weight` and `line-height` (`kernel/src/arena/control_text.rs:26`). Previously the shorthand wrote both rows, so they were copied from the stored mask before any start. `button font="-exact-headline"` on a host that installs control fonts (Apple `host/apple/src/button.rs:187`, Linux `host/linux/src/paint/control.rs:39`, weight 400) computes and paints the control face (`kernel/src/arena/button.rs:188`), not headline's 700 on the Mac or 600 on iOS, and the Mac leading `1.2308` becomes the boundary's `normal`. On the neutral column the same button loses both rows to the control line (`kernel/src/arena/control_text.rs:98`) even with no control fonts installed. Web with no control fonts has no weight in `WEB_START`, which is why the new test passes.

**[P1] A bound text style updates only font-size on the JS target — `host/web-js/src/style.rs:692`**

`TEXT_STYLE_MAP` (`host/web-js/src/style.rs:594`) and `style_writes` for `FontSize` emit a rem size. `template.rs` bakes weight and leading once from the initial style. `text font-size=(big ? "-exact-headline" : "-exact-body")` remeasures in the kernel: a relative-unit change is a style change even when both are 13px (`kernel/build.rs:1123`), and on the Mac the weight moves between 700 and 400 with the leading staying `1.2308`. The page keeps whichever weight was emitted first. The same gap is `font=(cond ? "-exact-headline" : "-exact-body")`: lowering binds `font-style` and `font-size` and a constant `fontShorthand`, not the two rows. A static `font="-exact-headline"` still agrees.

**[P2] A written clear does not stop a bare style's fill — `kernel/src/arena/defaults.rs:328`**

The end fill and `computed_source`'s bare return (`kernel/src/arena/defaults.rs:236`) ignore `written`. A shorthand is safe: its start is skipped when the row is written, and `bare_text_style` returns none for a shorthand. A bare name is not. `text font-size="-exact-headline" font-weight="unset"` with no ancestor weight is a written clear (`runner/src/instance.rs:1070`), so the weight should be the CSS initial 400. Both `computed` and `style_given` supply the headline weight (600, or 700 on the Mac). The same is `line-height="unset"`. `font-weight=300` beside the name still wins, and that case is tested.

Block-level `copy_inherited`, `computed_source`, `text_style_source`, and the agent `style` / `heading` provenance match the ruling. The `font` warning index is 1, matching `[font-style, font-size]`. Nothing left reads the old shorthand weight or `line-height: normal` rows. A font-size that exists only on a start is a pixel size (`kernel/src/arena/control_text.rs:29`), so it cannot name a style. Components use the same tree: a `font` on the component root is a normal shorthand; an inline component text hits the first finding. `fontShorthand` is not `measure: true`. Nothing in this commit changes it after the first layout (it is a constant implied prop), so the paragraph cache does not go stale now. A later dynamic write of the prop would bump paint only (`kernel/src/txn.rs:724`). `HEADING_BOLD` and Bold Text (D12.3) are the provisional ruling, not this commit.

Tests never cover an inline span, a button or field with control fonts installed, a bare parent plus a child with its own `font-size`, a dynamic `font-size` or `font`, or `unset` beside a bare style.
