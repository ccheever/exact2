# RFC review: LLP 1099, UIKit's springs everywhere (r9, `9eaea3c8a`), 2026-10-05 (astra, round 9)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `9eaea3c8a`.
- **Method:** the round-9 brief (sha256 `186e3bd1c346db1bb85a0cf7c9f9a561480502be974d5451ffb4f6b3220ee9f1`): check the round-8 dispositions, then review afresh; blind to the other round-9 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** SOUND WITH CHANGES.
- **Disposition:** LLP 1099 r10 §11 (round 9; unreviewed, the last approved round).

---

SOUND WITH CHANGES

1. **MATERIAL — The new paint refusal test contradicts its algorithm.** [D6:589](llp/1099-uikit-springs.rfc.md:589) and [§8.4:1086](llp/1099-uikit-springs.rfc.md:1086) require refusing `background-color 1s spring(1e10, 2e5, 1)`. My Bun reproduction of the specified curvature grid produces **46 stops, including the terminal pair**, below 64. The maximum curvature certificate is **1/510**; sampling 999 points per interval finds maximum error **0.00075744**. Round-8 A1’s fidelity fix works, but its refusal expectation does not. **Fix:** accept this case and test its between-stop fidelity; retain an actually over-64-stop case for refusal.

2. **MATERIAL — Handoff from an easing to a component-form spring remains undefined.** [D5:503](llp/1099-uikit-springs.rfc.md:503) converts only a live *restart spring*. Yet [D5:450](llp/1099-uikit-springs.rfc.md:450) measures new components from the old target. For `translate 1s linear`, 0→100, interrupted halfway toward 200 with `spring(400ms, .8)`, that rule starts at **100**, while presentation is **50**. Preserving the easing or collapsing it gives different trajectories; neither handoff is specified. Today [engine.rs:450](motion/src/engine.rs:450) samples either curve before replacement. **Fix:** define easing→component handoff from the sampled presentation, explicitly choose velocity handling, and add a cross-host regression.

3. **MINOR — Specify the promoted spring’s time anchor.** [D5:509](llp/1099-uikit-springs.rfc.md:509) preserves current residual, derivative and end, but omits its pending delay. Promoting `spring(100,10,1)` during a one-second delay can therefore preserve that delay or start component 0 immediately. With residual −100 and zero velocity, the latter reaches **−89.55945 after 50 ms**, while the former remains −100. **Fix:** explicitly preserve the original start and absolute end deadline, and test promotion during a delay.