# Code review: the server half of stage 11 D (base names on the wire, tamber/1.14), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e9b1291f3c`.
- **Method:** one brief (sha256 `6a36bb1e6749d89950ace9daadcb41fecff3b0c1490e157250fdadc9a6b1c2c0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**I found blockers; I would not land this wire change yet.**

- **Blocker — [tamber_base.rs:40](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:40): invalid or unknown designs become “no override.”** Missing files and parse failures both become `None`. Consequently, base `resolve`/`explain` succeed for `file: "missing.tamber"` or `"app.contract"`. A syntax error elsewhere also makes an existing override disappear from the answer. A when-only declaration is invalid syntax here and suffers the same problem. **Fix:** preserve file-membership and parse/check errors; distinguish a valid design without an override from a design that could not be read.

- **Blocker — [tamber_base.rs:126](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:126): `bases` misses valid references.** Package tokens and params are absent from the raw selected design’s `tokens`, despite existing explain successfully resolving them. Thus a package’s `reading.pad = space.md` or a param default of `space.md` gets no `bases`. The early return also omits it for an overridden base such as `space.md = space.sm`. **Fix:** derive references from the resolved declaration in the checked, merged design, including params and overridden bases, before returning.

- **Should-fix — [tamber_base.rs:94](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:94): base dispatch bypasses param resolution.** A valid component named `space` with param `md` previously answered `resolve`/`explain` of `space.md` as that param. These wrappers now return the base instead. If the design also overrides the base, delegation can combine the param’s value with the base token’s override metadata. **Fix:** preserve `lookup`’s param resolution and ambiguity handling before deciding whether the request names a base.

- **Should-fix — [tamber_base.rs:138](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:138): text splitting invents references inside strings.** For example, `family.body: family = "space.md"` is a literal font-family name, but explain adds `bases: ["space.md"]`. The checker does not treat it as a base reference. **Fix:** collect semantic references, including bare unset bases, through the checker rather than scanning arbitrary text.

- **Should-fix — [tamber_base.rs:77](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:77): replies mix snapshots.** `describe` keeps the original IR and `files` digests, then reads everything again for overrides. An intervening editor save produces new override text with old values and digests. Overridden-base resolve/explain and ordinary explain have the equivalent problem. This also adds another complete snapshot read/hash and parse. **Fix:** share one snapshot and parsed/checked design across the original response and its additions.

- **Should-fix — [tamber_base.rs:85](/tmp/rv-s11d-1/contract/cli/src/tamber_base.rs:85): unmapped shipped targets silently disappear.** `shipped_targets` includes manifest target keys, but `filter_map` drops any whose column is unknown. The result no longer matches the target set used by icon `draws`. **Fix:** report an explicit unmapped-target error instead of dropping it; add a regression case.

- **Should-fix — [scale.rs:98](/tmp/rv-s11d-1/kernel/src/style/scale.rs:98): the terminal mapping gets ahead of the host.** At this HEAD, [terminal boot](/tmp/rv-s11d-1/host/terminal/src/host.rs:227) never selects `Column::Terminal`; the default column reads neutral scale values. The new metadata therefore does not describe what that host currently reads. **Fix:** land the terminal column selection before advertising it, and test the host’s actual selection.

By inspection, the normal row serialization is correct: nine names in schema order, all five columns, correct source fields, graphical `radius.full: "full"`, terminal radii `"0"`, and correct override file/line/column/value text for a valid selected file. Exact-name matching, `space.md2`/`x.space.md` exclusion, default-before-when ordering and deduplication also look correct. Ordinary non-base requests retain their existing delegation apart from the issues above.

The tests cover the happy paths, but [the cell assertions](/tmp/rv-s11d-1/contract/cli/tests/it/tamber_serve_base.rs:55) compare only `space.md` values and merely require source kinds to be strings. They miss measured/guideline payloads, terminal zeroes, other-file overrides and the failures above. [The host-source test](/tmp/rv-s11d-1/contract/cli/tests/it/tamber_serve_base.rs:115) checks substring presence; swapping Apple’s branches would still pass. Expand assertions to all 45 cells and exact source objects, plus semantic reference and dispatch edge cases.

Both permitted Cargo commands failed **before compilation** with sandbox `Operation not permitted` errors creating temporary `target…` directories. No test results were obtained.
