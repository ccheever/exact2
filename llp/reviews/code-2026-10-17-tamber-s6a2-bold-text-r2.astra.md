# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the change and its first round.
- **Method:** one brief (sha256 `f1d0a9cfa0ec69d8bd426ab88e9c94b01ca134a1af727e6bf0849cd1aec8e07c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P2s: UIKit's control family and a stepped control default, Android fields adjusted twice, the button face discarding the step, runtime generic stacks, Markdown's bold).
- **Disposition:** in the second-round commit.

---

Five P2 issues remain; I would fix these before landing.

- **P2 — UIKit control fonts bypass authored-weight stepping.** [text_style.rs:120](/tmp/rv-s6a2-2/kernel/src/arena/text_style.rs:120): UIKit’s registered control-family ID sits beyond the plan’s stacks, so `system_family` rejects it. A native input with `font-weight=400` remains 400 under Bold Text. Conversely, explicitly choosing `system-ui` can step an already-bold control default again.

- **P2 — Android fields receive two adjustments.** [arena.rs:633](/tmp/rv-s6a2-2/kernel/src/arena.rs:633) steps a regular field to 600, then [Fields.kt:794](/tmp/rv-s6a2-2/host/android/canvas/src/main/java/dev/exact/canvas/Fields.kt:794) assigns that typeface to `EditText`. Android’s [TextView implementation](https://android.googlesource.com/platform/frameworks/base/+/main/core/java/android/widget/TextView.java) adds `fontWeightAdjustment` again: with adjustment 300, the field reaches 900.

- **P2 — Button-face processing discards the step.** [button.rs:199](/tmp/rv-s6a2-2/kernel/src/arena/button.rs:199) copies the button’s raw authored weight over its computed title weight: an Android/Linux button with `font-weight=600` paints 600 instead of 800. The mask reset at line 202 also removes an otherwise-unsaid stepped weight, causing default titles to revert in the painter.

- **P2 — Runtime generic stacks remain unweighted.** [root_font.rs:52](/tmp/rv-s6a2-2/runner/src/runner/root_font.rs:52) registers only compiled stacks. A bound `font-family` resolving to `"system-ui, sans-serif"` receives a runtime ID outside that table and remains 400, while the equivalent literal becomes 600.

- **P2 — Markdown expansion overwrites stepped weights.** [arena.rs:685](/tmp/rv-s6a2-2/kernel/src/arena.rs:685) steps before host expansion. Both Apple and Linux subsequently replace strong/heading weights with 700. For Markdown authored at weight 700, ordinary words become 900 while `**bold**` stays 700.

No additional issue found in SVG/strut stepping or touched-node invalidation. Static review only; no builds or device tests run. Worktree unchanged.
