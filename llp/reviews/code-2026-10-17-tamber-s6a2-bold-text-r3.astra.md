# Code review: Bold Text weights the system font (LLP 1115.000 D12.3, Tamber stage 6 A2), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~2..HEAD, the change and its first two rounds.
- **Method:** one brief (sha256 `ce8351529095060bd1534348f1e000afcbb6d2489b4f5c92d250c25adbfbcb59`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: Android's default controls exempted, the Android measurer stale after a live toggle; P2s: light weights under subtraction, Apple's Markdown classifying generic lists).
- **Disposition:** applied without re-review (the last round), in the third-round commit.

---

Four issues remain; the Android regressions block landing.

- **P1 — [text_style.rs:182](/tmp/rv-s6a2-3/kernel/src/arena/text_style.rs:182): Default Android controls skip Bold Text.** The exemption assumes every host control weight is already adjusted, but Android’s shared host registers fixed weight **400**. Launch with Bold Text enabled: ordinary fields and buttons remain 400 instead of 600. Restrict the exemption to control fonts actually adjusted by their host.

- **P1 — [Fields.kt:797](/tmp/rv-s6a2-3/host/android/canvas/src/main/java/dev/exact/canvas/Fields.kt:797): Live toggles leave measurement stale.** The detached, thread-local measuring `EditText` never receives configuration changes. Subtraction reads the new resource adjustment, while [TextView applies its cached adjustment](https://android.googlesource.com/platform/frameworks/base/+/main/core/java/android/widget/TextView.java). After toggling from 0 to +300, an authored 400-weight field targets 600 but measures at 300 while painting at 600, breaking widths and wrapping. Refresh or recreate the measurer.

- **P2 — [Fields.kt:799](/tmp/rv-s6a2-3/host/android/canvas/src/main/java/dev/exact/canvas/Fields.kt:799): Compensation cannot represent light weights.** With adjustment +300, authored system weight 100 should become 200. Subtraction produces −100, clamps to 1, and TextView adds 300 back: **301 instead of 200**. Arithmetic subtraction cannot cancel the adjustment across the supported weight range.

- **P2 — [DisplayPreferences.swift:192](/tmp/rv-s6a2-3/host/apple/Sources/ExactKit/DisplayPreferences.swift:192): Apple Markdown misclassifies generic font lists.** `system-ui, sans-serif` receives a nonempty catalog entry, so this predicate returns false despite the kernel classifying it as system text. With Bold Text enabled, `Some **bold**` measures and paints the bold span at **700 instead of 900**. Classify the stack’s first member consistently with the kernel.

Static review only; no builds or tests run. Worktree unchanged.
