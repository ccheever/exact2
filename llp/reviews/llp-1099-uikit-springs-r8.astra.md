# RFC review: LLP 1099, UIKit's springs everywhere (r8, `d95593101`), 2026-10-05 (astra, round 8)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `d95593101`.
- **Method:** the round-8 brief (sha256 `a658122f10306ec8aa5ac009ec3da614a2a2137194024dda4195c3d563c211e5`): check the round-7 dispositions, then review afresh; blind to the other round-8 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** SOUND WITH CHANGES.
- **Disposition:** LLP 1099 r9 §11 (round 8).

---

SOUND WITH CHANGES

1. **MATERIAL — The new paint sampler can miss almost the entire spring.** [D6’s fixed grid](llp/1099-uikit-springs.rfc.md:569) accepts `background-color 1s spring(10000000000, 200000, 1)` with four stops including the terminal pair. My Bun reproduction gives sampled error below **7×10⁻¹⁰**, but at **50 µs** the physical curve is **0.959572** and the approximation **0.20485**: approximately **192 channel units** on black→white. The compositor frame check passes with 81 frames. Sharing these stops across hosts makes parity pass without checking spring fidelity. **Fix:** certify error between stops using curvature bounds, enforce the 64-stop limit on that result, and add a test comparing the approximation with the closed form between fitting samples.

2. **MATERIAL — Resource refusals conflict with preserving existing springs.** [D1](llp/1099-uikit-springs.rfc.md:203) promises unchanged zero/three-argument parsing, while [D6](llp/1099-uikit-springs.rfc.md:638) mandates the new frame refusal at parse. Currently valid `translate spring(20000, 0, 1)` runs to the 10-second cap; the prescribed normalized spacing requires **22,363 frames**, so D6 rejects it. Similarly, today’s default physical paint lowering produces **117 stops**, conflicting with the unqualified 64-stop refusal while D6 promises to retain that lowering. **Fix:** explicitly scope the new limits to the new forms, or withdraw the unchanged-behavior promise and document the acceptance changes. Pin both examples in parser tests.

3. **MINOR — Clamp crossings are bracketed using the wrong values.** [D6](llp/1099-uikit-springs.rfc.md:680) searches between frames whose *displayed* values straddle a bound, although [the preceding rule](llp/1099-uikit-springs.rfc.md:671) already clamps those values. Displayed heights cannot straddle zero. **Fix:** explicitly bracket using the **unclamped engine values**, then clamp the inserted crossing frame; retain the negative-to-positive height regression.