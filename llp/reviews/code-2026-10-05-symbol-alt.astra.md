# Code review: symbols decorative unless named (exact-web test, JS target, LLP 1007/1011/1035.004), 2026-10-05 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox; round 1 at `c9f247102` (reviewing `bdff631bd`), round 2 at `cb36c9e28` (`bdff631bd` + `2c5ea3d5b`).
- **Method:** one brief per round, the same sent to grok (sha256 `735d11c0903cda0814e8f7bb80f316d2c2b8a49c86da90dd9911d91d6db0b3bb`, `3e404592b522d8d5ef13c4230f1124669c0e74f11c9c27b995fa3aa2f4e09002`); blind. Requested by the coordinator (Charlie approved). The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** round 1 LAND WITH FIXES; round 2 LAND.
- **Disposition (round 1), taken in `2c5ea3d5b`:**
  1. The test stays at the file's 1,499 lines (the two alt assertions are one line).
  2. LLP 1011 §3 says the web supplies `alt=""` or the author's name. The iOS follow-up it notes (`NodeViewIOS.swift` hides every symbol) is recorded in LLP 1011 §2 and 1035.004 D1, not fixed here.

---

## Round 1

LAND WITH FIXES

1. **P1 — host/web/tests/it/host.rs:105:** This grows the file from 1,499 to 1,502 lines; `caps.mjs` counts 1,503 and would fail the mandatory 1,500-line cap. Remove at least three lines or relocate a test.
2. **P3 — llp/1011-image-v1.spec.md:155:** §3 still unconditionally promises `alt=""`, contradicting the revised semantics. Qualify it with “unless the author named the image.”

Reading c9e5b9d17 as intended is correct—it explicitly names this behavior. The JS guard preserves names across symbol/app/plain source changes; bound labels independently update `alt`; templates already distinguish named and unnamed symbols. The regression test catches the original overwrite, though binding and transition coverage would strengthen it.

macOS agrees. iOS still forcibly hides named symbols at **host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:449**; follow up by deriving wrapper accessibility from its name while keeping the drawing child decorative. This predates the commit and does not block this repair. Linux exposes no AT-SPI tree.

No builds or tests run.
## Round 2

LAND

No remaining findings. Source and label transition probes passed. Native differences predate these commits and are now documented. No edits or builds performed.