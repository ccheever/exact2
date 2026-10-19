# Review round 2, LLP 1121 stage 11 B1 (the base scale table), Grok (grok 4.7, xhigh)

**RIGHT**

Round 1 is fixed. Astra's four items hold in the tree and in the tests that were run: a bare unset base name is an alias (a `when` arm too); `tamber-base-kind` covers icons; the kernel build requires a `docs/scale-measured.md` entry and a `Binds:` line for that step and that number.

Where the design sets the base name, the alias folds like any other token reference. `card.pad = space.md` with `space.md = 40px` folds to `40px`, records `space.md` in the token's names, and re-folds to `48px` on the next check. `session::changed_tokens` diffs every folded value, and an ordinary length change stays `Class::Live`, so the design server's batch carries the alias. An expression over an unset base name is `tamber-base-expression` (`calc` and `max`); over a set name it folds. A ranged length still has to be px, so an unset base name as a ranged default is refused and a set one checks in px.

What holds: the nine names only; same-role metric cells, with the Mac `space.xl` = 30 stretch and the neutral 4/24/32 steps documented; monotonic on all five looks with `radius.full` left out; `Column::Web` reads the neutral cell; an app length wins, including under an axis, and a colour, number, or icon is refused; retention keeps a read override and drops an unread one; `SCHEMA_DIGEST` is `0x374f_477d_d305_5b7e`, named from `0xad42_cc2d_c5b5_26f3`. The wire sample bytes in that test are unchanged. No protocol file is in the diff. `contract/lower/src/theme.rs` is 1496 lines.

Tests, `CARGO_TARGET_DIR=/tmp/rv-target-astra`: kernel `style::scale` plus `wire_codec_bytes_and_schema_digest_move_as_one_snapshot` (9), `base_alias` (1), `tamber_base_scale` (5). All passed.

No findings.
