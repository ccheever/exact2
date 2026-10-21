# Code review: exact-tamber crate, round 3 (the last), 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `72896cb91ad4c63e2baef74356c706d1fcf538bd983f38904364b0bd9de20658`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 2 MAJOR, 2 MINOR; Grok 2 MINOR).
- **Disposition:** all applied after the last round without re-review, each with a corpus case, and named in the commit message

---

I'll review only the round-2 fixes in `66696f054` against `72dc33da7`, plus anything those fixes broke. Reading the review brief, the diff, and the commit message first.The delta is 20 files. I'll read the fix diff against the RFC and the surrounding checkers, then run the crate tests.The diff matches the ten claimed fixes. Next I’ll read each changed function against the kernel and the RFC so a regression isn’t just a green test.Auto-compacting conversation (82% full)...
Conversation compacted.
The remaining questions are whether `named:` platform colours and hidden under-dark `when`s actually misbehave after the round-2 fixes. I'll confirm those against the current code and run the crate tests.The hidden-when check and `named:` platform names are the two places the fixes can still be wrong. I'll run the crate tests and a small in-memory check of both.The hidden-when base is still taken from tuple 0, and `named:` may never reach the new name check. I'll run both through `check` and start the crate tests.The hidden under-dark check compares a `when` with the all-defaults tuple, so a correct compact light side is refused and a wrong one is accepted. `ios named:Brand` still never reaches the new name check.

**Hidden under-dark base is tuple 0.** `unselected` always builds its base from `digits(0)` (`tamber/src/check.rs:597`), then `select`s while ignoring under-axes (`check.rs:607`). The commit’s own rule is that the light side is the value selected while ignoring under-axes, so a compact density’s light side is kept. That is what a selected `when` does. A `when` no tuple selects does not: its own non-under conditions are ignored.

With `density=compact` painting `#eeeeee`, brand values covering both sides, and a shadowed `when dark-style=dim density=compact: light-dark(#eeeeee, #222222)`, the check reports `tamber-under-light` (`#eeeeeeff` against the default `#ffffffff`). The same `light-dark` is accepted when that `when` is the one a tuple selects. The opposite shadowed value, `light-dark(#ffffff, #222222)`, is accepted, and the wrong light side is not the one any tuple keeps. `cargo test -p exact-tamber` does not cover this: `refuse-under`’s hidden `when` names only `dark-style`.

**`named:<asset>` is unreachable.** `native_name_ok` (`tamber/src/fold.rs:369`) accepts it, and the new `ios bogus` diagnostic at `fold.rs:344` tells the author to write it. The kernel accepts `-exact-platform-color(ios named:Brand, #c8102e)` (`kernel/src/style/roles.rs:893`; LLP 1095’s theme example is this form). A `.tamber` value fails earlier: `css.rs:139` reports `` `:Brand, #c8102e)` is not CSS value text ``. `web #fff` and `ios systemTealColor` fold; `ios bogus` is refused.

The other round-2 fixes hold. Percentages stay typed through `calc` (`calc(10 + 50%)` is refused; `calc(25% * 2)` and the legacy `calc(50%)` forms fold). Influence probes an interior hue and a translucent colour, and `tamber-dial-inert` runs only when nothing else was refused. Relative `oklch` components stay unclamped, a powerless hue is refused, both `color-mix` percentages of 0% fold to an even mix at alpha 0, and longer hue at exactly −180 matches `exact_color::mix`. Metadata escapes decode `\n \t \" \\ \` \$` and refuse the rest. `cargo test -p exact-tamber`: 7 passed.

VERDICT: fix (MINOR tamber/src/check.rs:597; MINOR tamber/src/fold.rs:369)
