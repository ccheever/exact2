# RFC review: LLP 1099, UIKit's springs everywhere (r5, `5afd8bd8f`), 2026-10-05 (astra, round 5)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort max, read-only sandbox, `-C` a detached worktree at `5afd8bd8f`.
- **Method:** the round-5 brief (sha256 `9edf0f4367cf89e854493f25bcedeed58bfb20e3795c349b2ac0511f5806a1d3`): check the round-4 dispositions, then review afresh; blind to the other round-5 review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited but for paths made repo-relative.
- **Verdict:** NEEDS REWORK.
- **Disposition:** LLP 1099 r6 §11 (round 5).

---

NEEDS REWORK

1. **MATERIAL — Zero-target scale releases remain undefined.** [D5’s scale rule](llp/1099-uikit-springs.rfc.md:379) makes presentation `target × product(factors)`, while the [release rule](llp/1099-uikit-springs.rfc.md:442) requires the actual release derivative, including at zero distance. For `spring(400ms, .8)`, releasing scale 0 toward target 0 at 1 scale-unit/s gives **0.018832 at 50 ms** under the specified textbook curve; every finite factor product instead presents 0. Round 4’s fix handles ordinary retargets, but leaves this release unresolved. **Fix:** define an absolute spring representation for zero-target releases, its subsequent retarget behavior, and a cross-host fixture.

2. **MATERIAL — Delayed component starts invalidate the sampling bound.** [D3](llp/1099-uikit-springs.rfc.md:313) delays each new component; [D6](llp/1099-uikit-springs.rfc.md:511) mandates breakpoints only at ends. A delayed start with nonzero velocity introduces a derivative jump that the [curvature envelopes](llp/1099-uikit-springs.rfc.md:527) cannot bound. Bun reproduction: animate 0→1 with `1s spring(1,0,1)`, then at 100 ms target 2 using the same spring with `velocity 100` and a 10 ms delay. Applying D6 gives **0.05503 units** interpolation error at 113.349 ms, versus the **0.002** parity allowance. **Fix:** split sampling intervals at every component start, preserve older motion during the delay, and test between frames across delayed starts.

3. **MATERIAL — Height clamping defeats the promised web tolerance.** [Web lowering clamps individual frames](host/web/motion-glue.js:98), whereas [native height clamps each sampled value](host/apple/src/height.rs:301). D6 bounds the unclamped curve, so these operations need not agree. For height 100→0 under `height 1s spring(100,10,1)`, I computed **0.37301 px** from the proposed web grid at 241.845 ms versus native **0**, exceeding `10⁻³·S = 0.1 px`. **Fix:** insert zero-crossing frames or bound interpolation after clamping; define the comparator against displayed height and add an undershoot regression.