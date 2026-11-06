# Active targets: Astra review, round 2

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex-cli 0.167.0`, reasoning effort max, read-only sandbox, a throwaway worktree at `14ca3dbede` on `70365218ab`.
- **Method:** one brief (sha256 `ba20e49dd9e3233eb082b3141e3f38eed25ea34ce6af6c924df831741738b8e8`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths and its commits name that frozen source.

---

Three **P2 findings** remain after the four independent passes on `14ca3dbedef` against `70365218ab`.

1. **P2 — Computed press scale stays stale during a stationary press.** The observer in [input-glue.js:475](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-targets-r2-ga6p3x3r/checkout/host/web/input-glue.js:475) refreshes paint, but scale is read only by `showPress`. A newly admitted `:active -exact-press-scale=(on ? 0.8 : none)` therefore remains unscaled when `on` changes while pressed. An isolated probe executing the driver reproduced both `none → 0.8` remaining at `1` and `0.97 → 0.92` remaining at `0.97`; another input refresh applies the target. Both web targets and held presses share this path. Style refreshes must also re-aim scale.

2. **P2 — A plain UIKit row retains its native highlight after authored paint appears.** At [PressFeedback.swift:131](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-targets-r2-ga6p3x3r/checkout/host/apple/Sources/ExactKit/PressFeedback.swift:131), `rowCells.press` runs only while `platformPressDims` is true. Start pressing a plain row with no active targets, then introduce computed active opacity: the row already has a `RowGroundView`, but authored paint disables that branch. The new cleanup clears dimming, not the row ground. Release also bypasses `rowCells.press(false)` and `RowCells.select`, leaving the gray highlight and hidden separators behind. Clear the existing row feedback when switching presentation.

3. **P2 — A trailing accessory overwrites the leading icon’s color ownership.** [GroupedPressIOS.swift:124](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-active-targets-r2-ga6p3x3r/checkout/host/apple/Sources/ExactGroupedLists/GroupedPressIOS.swift:124) assigns the single `image` protection flag for every image visited. In a standard grouped row containing an explicitly red leading symbol, text, and an uncolored disclosure chevron, the chevron overwrites that flag with `false`. Active blue ink then converts the leading image to template rendering and replaces its authored red. Determine ownership from the actual leading symbol used by the content configuration.

I **do not agree that every R1 settlement is complete**. The basic grouped-cell wiring and late background/opacity handling are present, but the cases above remain. I agree with separating Linux feedback from the activation route chosen at down, including cancellation cleanup. No additional material findings emerged in AppKit, simplification, or writing.

Verification and limits:

- Independently reran the four new compiler regressions, 25 native-button tests, and 48 focused Linux tests using existing binaries: all passed. Three other state-part tests were blocked by read-only temporary-file restrictions.
- Reproduced the web scale failure with a bounded JavaScript probe. The UIKit findings follow source and caller paths; I did not perform native pixel verification.
- Full gates are **not green**. Supplied Linux controls reproduce the exit and snapshot-retry failures only on the candidate, including crossed working directories; their causes remain unresolved. Root tests also failed.
- Candidate and main web app logs both pass authored tests and fail the same iframe-tree smoke assertion. The iOS app reached signing but failed simulator boot. Caps reports the existing `foldables.md` violation.
- No source changes, Firefox, GUI launches, or fresh Cargo/Swift builds were performed.