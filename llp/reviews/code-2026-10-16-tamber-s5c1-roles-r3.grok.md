# Code review: per-session role tables (Tamber stage 5, commit 1), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the second round over the change.
- **Method:** one brief (sha256 `04322f1dad5d2acff0a74edb60ff2014efa4ca7ef9375812ed1227033d1c9707`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** applied without re-review (the last round).

---

I'll review round 3 of the per-session role tables as a read-only pass: the brief, the round-2 diff, and whether each finding is actually fixed.Round 2's stack, sheet adoption, and owner cleanup hold. Two defects are still real.

**[P1] ExactIOS no longer builds — `host/apple/Sources/ExactKit/IOS/NodeExtrasIOS.swift:271`**

`platformBackground` is now `platformBackground(in:)`, and `host/apple/Sources/ExactIOS/main.swift:131` still calls `.platformBackground`. ExactIOS is an executable in the same package, so this is a missing-argument error. Host tests never compile that target, which is why they stayed green. `bun host/apple/build.mjs --ios` fails before launch.

**[P2] A box-dependent gradient drops tint-following roles — `host/apple/src/gradient.rs:96`**

`gradient_json` bakes a stop only through `overridden(id)`. Tint roles (`LinkText`, `ButtonText`, `-exact-link`, `-exact-symbol`) are not table entries; they follow `AccentColor` in `painted_override`, and `ColorValue::resolve` recurses. A px or repeating stop stays the role name. `exact_gradient` then binds an empty table (`gradient.rs:32`), so the presenter paints the platform colour.

One session, `AccentColor = "#ff0000"` and `linear-gradient(90deg, LinkText 0px, transparent 100px)`, paints platform link blue. The same stop without a length paints red. A role overridden to a tint (`CanvasText = "LinkText"`) is the same hole. A direct `AccentColor` stop is baked and is fine.

The out-of-order `Installed` stack is sound: `uninstall` removes only that session, and `bound` skips a dead weak. `adopt(_:from:)` is used from the dialog sheet, the modal sheet, and the Mac sheet with the presenter's root, including when the view loads outside `within`. Grouped lists, bars, and `platformBackground(in:)` pass that root, so two embedded sessions do not walk into each other. Owners whose views are gone are dropped on the next `own`. No role-table leak.

`a_refused_candidate_and_a_nested_boot_leave_the_live_sessions_roles` does what it says (first candidate dropped while the second is still installed). `resolved` checks the real `with_entry` and `with_runtime` binds; removing the colour-report test is fair, because that test could not see a wrong bind, and `exact_colors` uses the same bind. `a_boxed_gradient_carries_its_sessions_accent` checks that each boot batch contains its own hex and that resolving the raw `AccentColor` CSS is not the other session's blue. It never resolves the baked CSS, and it would still pass if that resolve returned the first session's red or a platform colour. It does not cover a tint-following stop.
