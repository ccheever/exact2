# Code review: Bindings of lengths, numbers and fonts, and the DTCG export of every kind (LLP 1121 stage 6 G), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD, the export and the bindings.
- **Method:** one brief (sha256 `2c265923b543f12b10432c2a771874a79c882759c057d3dc0907edea1d0dccdf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: a native module's system font falling back, a reprinted number discarding sources; P2s on cached lengths, an `em` leading, unsaid parts, scheme shadows, units, stale tuples, motion unions; a P3 on re-proposing).
- **Disposition:** in the first-round commit (the system font in the sample-host commit before it).

---

Blocking issues remain.

- **P1 — Native-module fonts fall back incorrectly.** [NativeHatches.swift:190](/tmp/rv-s6g-1/host/apple/Sources/ExactKit/NativeHatches.swift:190) exports only size and font name. A CSS-weighted system font such as `600 18px system-ui` produces a private variation name that `NSFont(name:size:)` cannot reopen. Confirmed on this Mac: it returns `nil`, so the module uses its fallback.

- **P1 — An unedited JSON round trip can discard sources.** [export_kinds.rs:390](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:390) uses the caller’s colour-only equality check. `JSON.parse`/`JSON.stringify` changes `27.0` to `27`, falsely marking a dimension changed. Re-import then proposes replacing `-exact-title1` with `27px`; similarly, a lossy easing can become a literal linear curve.

- **P2 — Cached lengths miss dependency changes.** [runner/tamber.rs:396](/tmp/rv-s6g-1/runner/src/runner/tamber.rs:396) hashes neither the node’s font size nor display scale. Changing a node from 16px to 24px leaves a cached `1em` binding at 16, without notifying observers. Hairlines similarly remain stale after a display-scale change.

- **P2 — Font leading resolves `em` against the wrong size.** [runner/tamber.rs:305](/tmp/rv-s6g-1/runner/src/runner/tamber.rs:305): `20px/2em` requested at a 16px root returns a 32px line height, although the token’s font requires 40px.

- **P2 — Previously omitted typography fields cannot be edited.** [export_kinds.rs:311](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:311) obeys baseline `unsaid` metadata even when the incoming property changed. Setting exported `type.body = -exact-body` to weight 700 drops that edit, then fails validation. Omitted family and leading have the same problem.

- **P2 — Scheme-dependent shadows cannot be re-imported after editing.** [export_kinds.rs:397](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:397) requires identical light/dark objects. For a shadow using `light-dark(#000, #fff)`, changing its blur in both contexts is rejected because its colours differ.

- **P2 — Valid duration-unit conversion is rejected.** [export_kinds.rs:490](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:490): importing `0.2s` reads successfully as `200ms`, but validation compares their JSON representations and rejects the proposal.

- **P2 — Unchanged tuples are checked against stale exported values.** [export_kinds.rs:486](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:486): export regular=12/compact=8, change the current design’s compact value to 6, then import regular=14. Validation demands the unchanged exported compact=8 and rejects the independent regular edit.

- **P2 — Two token-kind unions are missing.** [typescript.rs:181](/tmp/rv-s6g-1/contract/cli/src/typescript.rs:181) generates only seven kinds. Declared/exported duration and easing tokens produce no `DurationToken` or `EasingToken`; valid imports fail `tsc`.

- **P3 — Re-importing an accepted edit proposes it again.** [export_kinds.rs:497](/tmp/rv-s6g-1/tamber/src/export_kinds.rs:497) returns operations even when applying them leaves `current` unchanged. Accept a length change from 12px to 6px, then import the same file again: another identical `SetToken` is proposed.

Worktree and index unchanged. AppKit reproduction completed; build suites were not run.
