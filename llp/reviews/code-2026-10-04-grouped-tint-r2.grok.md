# Code review, round 2: grouped list (iOS) symbol tint (461e80f95), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `461e80f95`.
- **Method:** one brief (sha256 `4beca76a9aa20d4f095c3d0a6fdd8a13c1ed67a6108e239f9946b8d157e5ea9d`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's output after its progress lines, unedited.
- **Verdict:** LAND.

---

I'll review the grouped-list symbol tint change statically, starting with the review brief and the round-1 findings so I can check each one against HEAD.Round-1 reviews are in. Next I’ll read the current lookup, the kernel’s symbol rules, and the test that claims to cover them.Round 1 is resolved. The appearance tint, the symbol lookup, the dropped background change, and the test all match the dispositions, and the four new suspects are not defects.

**Appearance — resolved.** `configure` installs a dynamic `UIColor` that reads the captured pair with the traits it is given (`GroupedListIOS.swift:352-356`). Destructive and disabled still assign `.systemRed` and `.tertiaryLabel` after that (`:359-367`). `looks` stores the raw style value (`:274-276`, `:448-450`), so a scheme change does not need another batch.

**Which symbol — resolved.** `symbolView` skips `display: none` and uses the first remaining child (`:443`). That child has to be a `symbol:` image, and an `sf/` name has to equal `row.symbol` (`:444-446`). A hidden image in front no longer supplies the tint.

**Background removal — resolved.** `configure` no longer assigns `backgroundConfiguration` (`:335-368`). A custom row keeps the list cell’s fill, which is what §6.1 now says (`llp/1084-native-grouped-lists.rfc.md:202-204`). The comment that had drifted onto `symbolView` is back on `carry` (`GroupedListIOS.swift:452-454`).

**Test — resolved, and it is meaningful.** `testASymbolTakesItsAuthoredTintForEachAppearance` (`GroupedListIOSTests.swift:183-202`) lays the cell out with a nil tint, then adds a hidden red `sf/xmark`, the model’s `sf/person.circle` with a light-dark pair, and a `forward-chevron`. The light side is black, so the hidden image was not used. The dark side is white with no further batch, so the dynamic color resolves both ways. The chevron has no `tint_color`, so selecting it would fail the unwrap. The model row is unchanged, so the second read goes green only because `tints[id] != looks[id]` reconfigures.

**Dynamic `UIColor` capturing a `BatchValue` — not a defect.** `value` is the style value copied at `configure`: either four channels or a light-dark pair (`Batch.swift:21-25`). The closure picks a side from the traits argument. It does not freeze one appearance. A later edit replaces the provider because `looks` stringifies that same value (`:448-450`), and `String(describing:)` for this enum includes the payload. `applyStyle` replaces the dictionary rather than mutating it, so the captured pair cannot be overwritten in place.

**`symbolView` against `kernel/src/grouped.rs` — not a defect.** `apple_symbol` (`grouped.rs:80-92`) turns `sf/<name>` into `<name>` and a role into the table’s Apple name. The host already stores that Apple name on `symbolName` (`host.rs:1435-1447`), and the kernel stores it on `row.symbol`. The Swift check is looser for roles: a non-`sf/` source is accepted without comparing `symbolName` (`GroupedListIOS.swift:446`). It still tints the right image. The kernel’s symbol is the first shown child after the trailing accessory is removed (`grouped.rs:166-196`), and that accessory is the last child, so removing it does not change which child is first. The only one-child accessory row leaves `row.symbol` nil, and `symbolView` returns nil on that guard (`:442`). A role such as `symbol:person` (Apple name `person.crop.circle`) is the first shown child and is accepted.

**`looks` — not a defect.** It is rebuilt from the current snapshot’s ids (`:274-276`), so removed rows drop out. New ids are insertions, not reconfigures (`old[id]` is nil). A retained row reconfigures when the model row changes, the old row was custom, or the tint string changes. A switch that is mid-flip still reconfigures on the next turn (`:280-288`), after `looks` already holds the new string. `configure` reads the view again then.

No findings.

Verdict: LAND
