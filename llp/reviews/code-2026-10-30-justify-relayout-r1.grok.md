# Code review: web hosts follow a block parent's justify-items (progress), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `/tmp/dt3-rv-c1`.
- **Method:** one brief (sha256 `d8c210e5dc64b059eb8f093f62c6797ef58a18a8b4d548b7368707b7dcd735ee`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the justify-items fix read-only: the kernel change, both web hosts, and whether the tests actually cover the bound path.The diff is five files. Next I’ll read both host paths and how parent style changes rebuild a child’s CSS.The direct case is right: both hosts force `justify-self: start` only for `normal` and `stretch`, which is the pair Chrome actually stretches. Two holes remain.

**[BLOCKER]** Stale browser test still expects a block parent’s `justify-items: center` to pin the progress at the start — `host/web-js/document-render.test.mjs:709`

`Rust-rendered spinners follow parent display after adoption` puts a `width=20` progress under `display=(grid ? "grid" : "block")` and `justify-items="center"`, and expects `{ x: 0, width: 20 }` while the parent is a block (lines 728 and 732). Adoption strips the baked `justify-self` (`rt.js` `adopt`), and the new rule resolves it to `auto`, so that box centers at x 110, the same figure the test already expects in the grid frame. The `justify-self:start` string check still passes because the other spinner has no `justify-items`. This test goes red.

**[MAJOR]** `--exact-progress-aligned: auto` inherits into a nested progress whose own parent does not align — `host/web-js/src/style.rs:450`

The property is written only when `justify-items` is not `normal` or `stretch`. Custom properties inherit, and a nearer block parent with the initial value writes nothing, so it does not cut the chain. A centered box that itself contains a progress, with a plain block descendant that contains another, leaves the inner one `justify-self: auto` under `justify-items: normal`, and it stretches. The bound writer already avoids this by setting `initial` (guaranteed-invalid, so `var()` uses `--exact-progress-justify`). The static path should write that same `initial` whenever the parent has a progress child. The wasm host does not have this hole: it stamps `justify-self` from the immediate parent.

**[MINOR]** The bound writer treats values the justify-items writer rejects as aligning — `host/web-js/src/rows.rs:532`

`gridValue` turns `legacy`, `last baseline`, and the CSS-wide keywords into `unset` (`host/web-js/grid.js:20` and `:39`), which computes to `normal`. The new predicate only looks at `normal()`, so those strings are not `normal`/`stretch`/`""` and it sets `--exact-progress-aligned: auto`. The progress then stretches. Previously `justify-self: start` kept the 20px box, which is what the kernel does when it refuses the value.

**[MINOR]** The new JS test does not prove the bound writer — `host/web-js/src/emit_tests.rs:1000`

`bound.contains("--exact-progress-aligned")` is already true from the progress rule `var(--exact-progress-aligned, var(--exact-progress-justify, start))`. Deleting the `rows.rs` writer, or making it always emit `auto`, still passes. The static `center` / `normal` assertions are red without the fix. The wasm test is red without the fix too, but it only calls `host_css_of`; it never dispatches a justify-items change.

Checked and sound. Chrome’s block fixture stretches an auto width only for `normal` and `stretch`. `start`, `end`, `center`, `flex-start`/`flex-end`, `self-start`/`self-end`, `left`/`right`, `baseline`, and the `safe`/`unsafe` forms all shrink to fit, including under RTL, where `left`/`right` stay physical. Both hosts use that split and leave direction to the parent’s own `justify-items`. The wasm host does recompute: any style change touches the node (`kernel/src/txn.rs` `SetStyle`), and `refold` restyles progress children from the live parent, the same path as display. Absolute progress is unchanged (JS writes `auto`; wasm only forces `start` for static, relative, and sticky). `initial` is the right custom-property reset. Canvas still always forces `start`, as intended.

Verdict: not ready to land — the adopted-document test still locks the old block geometry, and the new custom property leaks onto a nested progress.
