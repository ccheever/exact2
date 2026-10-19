# Review round 3, LLP 1121 stage 8 E2 (Apple) + E3 (Linux/Android) held states, Grok 4.7 (xhigh)

**WRONG**

1. **BLOCKING** — A same-plan generation swap reports `gone` for `hold active` on a descendant, and the server ends the hold. `host/linux/src/presenter/held.rs:66`

`hold` climbs with `display.parent`. That parent is the last acknowledged picture, and only while `display.active` is set (`display_frame.rs:139`). `reload_module` calls `new_session` (`presenter.rs:591`), which changes the picture origin and leaves `active` true (`display_frame.rs:91`), so `parent` is `None` for every node until the next picture is acknowledged. `rehold` runs in that window (`delivery.rs:114`), before Android's `display_frame` and after the Linux loop's. A named node that is not itself the press handler stops at the first step, `hold` returns `gone`, and `hold_frame` reports it at the live revision (`held.rs:245`) and ends that revision (`held.rs:269`). The server drops the hold and will not replay it. Hover, focus, and `active` on the press node itself do not climb, so they survive. `a_new_generation_lets_a_hold_go_unsaid_and_takes_it_again` (`held_tests.rs:318`) never calls `display_frame`, sets `painted` by hand, and holds hover, so it stays green.

Climb the live kernel parent in `hold`, the parent `disabled_at` already uses. Add a test that `display_frame`s, `display_complete`s, holds `active` on `child`, installs a same-plan generation, and expects one `held` at that revision and the press paint, with no `gone`.

2. **SHOULD-FIX** — A hold frame for another plan replaces the frame a same-plan reload will take. `host/linux/src/presenter/held.rs:232` and `host/apple/Sources/ExactKit/Inspect.swift:188`

Both hosts store the frame, then return unsaid when `plan` is not the running digest, and they leave the paint alone. `rehold` (`held.rs:287`, `HeldState.swift:262`) then sees the foreign digest, skips, and says nothing. The plan digest did not change, so the server does not replay. The paint is gone with the old tree and the tool still shows the hold. Store the frame only when `plan` is absent or equal to the current digest. A mismatched frame must leave both the saved frame and the current paint untouched. The server replays when the digest actually changes.
