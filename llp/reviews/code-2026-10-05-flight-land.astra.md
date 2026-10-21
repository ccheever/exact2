# Code review: a spring flight lands as UIKit's spring animators finish, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `257c3c154bc65b38924bc209356b4909cb9a85eacdf48e170237ed865e9cf61d`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at origin/main. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2):** P2 taken: `land_after` is now the first grid time from which the unit spring stays within the bounds (scanning back from the 10 s cap; `None` if it is not settled there), so a slow crossing is not its end; a regression seeks `spring(1, 1, 1)` past its crossing near 6 s and checks it has not landed. P3 taken: `cargo fmt`. DEFERRED: `clock settle` still asks the engine's original rest time (it may advance farther than a flight needs).

---

**READY WITH CHANGES.**

- **P2 — The first qualifying sample can be a slow crossing, not settlement.** [flights.rs:50](/tmp/x20/host/apple/src/flights.rs:50): evaluating the spring equations for valid `spring(1, 1, 1)` gives displacement `−0.000826` and velocity `0.04947` at **6.029 s**, satisfying both thresholds. Its next excursion reaches **2.66%** of the travel. Moreover, [flights.rs:138](/tmp/x20/host/apple/src/flights.rs:138) lands unconditionally after that deadline: a seek directly to 6.5 s lands while displacement is **1.72%**, defeating the stated tolerance. Use a conservative bound on the remaining oscillation, and add a regression covering this spring with a seek past the crossing.

- **P3 — Formatting fails.** `cargo fmt --all -- --check` reports changes required at [flights.rs:50](/tmp/x20/host/apple/src/flights.rs:50), including the new struct initializer and landing condition.

The other paths look sound:

- **Start time and delay:** `start_flights` runs after advancing to the receipt time, floored at the current engine time ([host.rs:1244](/tmp/x20/host/apple/src/host.rs:1244)). Positive delay is correctly included. Negative spring delays are rejected by [transition.rs:136](/tmp/x20/motion/src/transition.rs:136); negative easing delays remain unchanged.
- **Interruption:** the new motion node starts from rest at zero. Removing the previous flight also removes its stored deadline, so `end_flight_of` cannot leave a stale landing behind.
- **Visual landing:** [FlightsIOS.swift:114](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:114) restores the view to its slot, geometry, radius, input and accessibility. The photo spring’s deadline is **0.370833 s**, with remaining displacement **0.000925**. The small-error argument holds for that curve; it does not hold generally for the crossing case above.
- **Agent clock:** explicit seeks use the correct engine clock. One caveat: [`clock settle`](/tmp/x20/host/apple/src/host.rs:828) still queries the engine’s original rest time, so it can advance farther—and cross more timers—than the new flight deadline requires.

All **three flight tests passed** using the existing binary containing the new test. [The new test](/tmp/x20/host/apple/tests/it/flights.rs:116) catches the Signal timing regression, but does not cover oscillatory settlement, spring delay or spring interruption. No fresh build or UIKit visual run was performed. No files changed.
