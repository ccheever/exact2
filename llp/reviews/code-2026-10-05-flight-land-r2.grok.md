# Code review r2: a spring flight lands as UIKit's spring animators finish, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `2e282a0f7acdb924846f029d91c491a3024b26f4ac81fa205a5dfbee7cba1d54`), shared with astra. Round 2, a delta review of the round-1 dispositions, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take; `clock settle` stays DEFERRED.

---

**READY.** No findings.

The scan in `land_after` is the settle time it claims. It keeps a time only when every 240 Hz sample from that instant through t = 10 s is inside `|displacement| < 1e-3` and `|velocity| < 0.05`, and it returns `None` when the cap sample itself is outside (`host/apple/src/flights.rs:50`).

For `spring(1, 1, 1)` the old false crossing is still there: at 6.029 s the unit spring is inside the band (`x ≈ -8.3e-4`, `v ≈ 0.0495`), and at 6.5 s it is back out (`x ≈ 1.72e-2`). The cap sample is also outside (`x ≈ -2.17e-3` at 10 s), so `land_after` is `None` and the engine stays active. A seek to 6.5 s therefore does not land (`host/apple/tests/it/flights.rs:176`). A seek lands only once `lands_at` is set and `now >= lands_at` (`host/apple/src/flights.rs:150`).

The photo spring is unchanged: first in-band sample at 0.370833 s (`x ≈ 9.2e-4`, `v ≈ -0.021`), and both the 240 Hz suffix and a 20× finer grid stay inside the band through the cap. That is still after the 0.34 s tick and before the 0.42 s tick.

The cap does not cut off an earlier settle. On a sweep of frequencies and damping ratios, a cap sample outside the band never had even the previous grid sample inside it, so the `None` gate does not skip a settled suffix. A land time of exactly 10 s is the same instant the engine snaps. The scan is at most 2401 closed-form samples, once per flight start.

`clock settle` still follows the engine's rest time. That was left as-is; this change does not make it worse.
