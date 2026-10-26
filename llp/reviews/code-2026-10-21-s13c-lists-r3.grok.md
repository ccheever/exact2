# Code review: @exact/lists, ListRow and SwitchRow (stage 13 C), round 3, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c958c4c060`.
- **Method:** one brief (sha256 `25487ca659bd0b8ba84e6cc1d0fffa679bdeeceb7861933b79a0cde1433d5081`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT
- **Disposition:** none needed (its residual, accessory value with an empty value, is the QUEUE'd static-versus-live gap)

**VERDICT: RIGHT**

No findings.

The destructive tint is a second `if` inside the symbol `if`, so the symbol stays one node at the front of the row. Both classifiers walk that nest, and the kernel sees whichever image is showing.

- `leading()` in `contract/lower/src/grouped/actions.rs` treats an `if` as a leading symbol when every branch is empty or one leading node. The inner `if destructive` is one image per branch (`symbol:${symbol}`, not an accessory). The outer `if symbol != ""` is that inner `if` or nothing. `action_cell` therefore still sees the accessory-`none` branch (a single symbol `if` plus a title, no `aria-selected`) as an action cell.
- `part()` in `contract/lower/src/grouped.rs` restyles both arms of an `if` at the same place in the row. Both images sit at the first shown slot, so they get the leading-symbol metrics. `inset()` walks the same nest, so a visible symbol still takes the symbol inset. The destructive image has no `-exact-tint-color`, so the sheet's destructive symbol tint applies. The other image's `param.tint` is an author row and replaces that sheet tint.
- The kernel reads shown children (`kernel/src/grouped.rs`). The taken branch is one image, the first shown child, and `apple_symbol` names it. iOS then finds that same first image. A destructive row's image tint is forced to the destructive red after any author tint.

D5 holds. A variant is a bool or enum read by a `class` condition or a style `?:` / `match` (`contract/lower/src/anatomy.rs`). `accessory`, `selected`, and `destructive` are only structural `if`s or attributes (`destructive=`, `aria-selected=`). Neither `ListRow` nor `SwitchRow` has a variant prop, which is why the gate goes from 11 cells to 13.

D1 holds. The only colour rows are `param.tint` and `param.on`. The param defaults are `lists.symbol-tint` and `lists.switch-on`. Those tokens are `AccentColor` and `-exact-system-green`, not literal lengths or colours. `SwitchRow`'s `on` param is priority 2, after the symbol tint.

The check row always sets `aria-selected`, so an unselected check is a choice on the sheet and in the kernel. The accessory-`none` branch is only a symbol and a title, and its symbol takes no param tint, so the platform accent or destructive red shows.

One residual, not a package defect: `accessory="value"` with `value=""` is the already-tracked sheet-versus-kernel split. The source still has the value `if`, so the sheet does not call it an action, while the kernel sees only a symbol and a title and does. A chevron row does not hit this, because the chevron remains. The new test locks the destructive symbol to the title's red and different from the plain row. It does not lock the plain symbol to `param.tint`, because that default is the same accent as the sheet's symbol tint.
