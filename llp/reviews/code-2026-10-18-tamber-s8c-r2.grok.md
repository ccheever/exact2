# Code review: LLP 1121 stage 8 C (the control-role table and tamber-unreachable), r2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree.
- **Scope:** `1a61196df..a6d4db4a5 (as reviewed: ebd43a6a3, before the rebase)` (C1 round 2, C2 round 1).
- **Method:** one brief (sha256 `650d423336ffc484ac2aa5d8f195d4dfd1c125cb8fccf33f53b5b8e52c5b0cc0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Disposition:** fixed in the pre-landing 8e0029eee (C1) and cfcef760e (C2), folded into the landed commits.

---

## BLOCKING

**macOS disabled checkbox and prominent button are `none`, so the lint warns, but the host still applies a tint.** `kernel/tables/schema.json:5799` (`checkbox:disabled` / `none: AppKit draws its own disabled checkbox`) and `:5806` (`button-prominent:disabled` / `none: AppKit draws its own disabled bezel`). `docs/control-roles-measured.md:39` says a Mac accent cell is `none` only where the control takes no tint, and `best-effort` where it takes one. `ControlsMac.swift:171` sets `contentTintColor` on the checkbox `NSButton` before `isEnabled` at `:176` — the same setter the enabled checkbox cell calls best-effort. `ButtonConfigurationMac.swift:83` sets `bezelColor` with no enabled check, and `NativeButtonsMac.swift:294` records that AppKit applies that disabled bezel tint. That is best-effort, which the lint must stay silent on. As recorded, every disabled Mac checkbox and disabled prominent Mac button with an accent warns. A disabled radio shares the checkbox setter but has no `:disabled` row (`schema.json:5800`), so it stays on the best-effort row and stays silent.

## SHOULD-FIX

**`control()` misses a prominent button whose style is not a string on the element, and treats a computed `type` as a text field.** `contract/lower/src/warnings/controls.rs:198` reads `buttonStyle` only as `Expr::Str` on the element's own attributes. `buttonStyle` is styleable (`kernel/tables/schema.json:1302`) and legal inside `style` (`contract/lower/src/lib.rs:351`), and the lowerer accepts a choice of names (`contract/lower/src/controls.rs:744`). A `filled` / `prominent-glass` button written that way is not `button-prominent`, so the iOS disabled-glass warning never fires. At `controls.rs:189`, a non-string `type` (the `None` arm) is classified as `field`, so a computed checkbox, radio, or range is judged against `Field` / `FieldText` / `placeholder`.

**The terminal column never warns.** `contract/lower/src/warnings/controls.rs:97` matches a shipped target by exact name, and `contract/lower/src/icons.rs:107` only adds crates `android/`, `linux/`, and `web`, plus `app.json` host / store / binaries keys. Apps ship the terminal as `terminal.contract` plus `terminal/Cargo.toml`, and there is no `host.terminal` key. `compile_path_terminal` (`contract/cli/src/lib.rs:565`) compiles with no warning buffer (`:648`). Every `none: the terminal draws no colour` cell is unreachable.

**A failed compile reuses the previous compile's targets and role spans.** `lower_all` always appends `with_text_styles` (`contract/lower/src/lib.rs:257`) even when `lower_with_sites` returns `Err`. `declare_fonts` (`:327`) and `declare_sounds` (`:328`) can fail before `declare_theme` clears the thread-locals (`contract/lower/src/theme.rs:211`). `warnings()` clears them (`contract/lower/src/warnings.rs:47`), but this path does not call it. The next file's `tamber-unreachable` then uses the previous app's shipped targets and the previous file's role spans. `compile_file_warned` (`contract/cli/src/lib.rs:856`) hits this whenever analysis succeeded and font or sound lowering failed.

## NIT

**Dead assertion alternative.** `contract/cli/tests/it/control_roles.rs:92` accepts a message starting `FieldText on the web's fields: none: `. The stored why has no `none:` prefix (`kernel/build/codec.rs:777`), so only the second `starts_with` can pass.

**Agent doc example is shorter than the lint, and it omits two exempting rows.** `docs/contract-for-agents.md:3110` stops at `NSSwitch draws the system accent`. The warning (`controls.rs:128`) and RFC C2 add `(the first at line N; LLP 1121 D9)`. The same paragraph says a field's own `color` or `background-color` wins. The code also treats `background` and `::placeholder color` as the field's own row (`controls.rs:17`).

**Class rows are cited ahead of the node's own.** `controls.rs:141` says the node's own rows win, but `rows()` (`:153`) puts class attributes first and `find` (`:66`) takes the first `accent-color`, so the warning span is the class when both set it. A class `color` also exempts `FieldText` the same way.

**Schema prose names the Mac column `mac`.** `kernel/tables/schema.json:5794`. The generated host is `macos` (`kernel/build/codec.rs:761`), which is what the lint matches.

Windows maps correctly when `host.windows` is set (Linux painter, marked inferred). tvOS is built as the iOS app and still gets the iOS accent path, so the `ios` column is the right one. Mac textarea `Field` = exact matches `TextAreaMac.swift:314`; the input does not read `Field`. Android `placeholder` = none matches the reader using `-exact-placeholder` (or a transparent floated label), not the `placeholder` role. `validate()` runs before codegen (`kernel/build.rs:1453`).
