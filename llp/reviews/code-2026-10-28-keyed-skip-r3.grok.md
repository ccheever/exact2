# Code review: runner keyed-row skip (x2/keyed-skip), round 3, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `17270cbd32`.
- **Method:** one brief (sha256 `6b4cf1092ccbfd9e50b9a1cf8509f1fb9606dae438a68272e790ea3b90158d88`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's two context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed

## Brief

Round 3 of 3 reviewing the branch: diff origin/main..HEAD, two commits, unchanged since round 2. Read-only: no checkout, restore, stash, reset or edits.
It is a runner change (exact-runner), runtime level. Round 1's brief has the change, the position audit and the measurements: /tmp/iosfix-rv-keyed1-brief.txt. Read it first.
Rounds 1 and 2: Astra's P2 (the fresh-build oracle) is fixed in 17270cbd32. Round 2 was CLEAN from both reviewers.
Round 3: try to BREAK it. Construct, on paper or as a plan sketch, a Contract program whose visible output after a prepend (or any reorder: a move to the end, a swap, a sort, a filter that removes rows above) differs from a fresh build of the new list with this change, where it didn't before. Cover:
- reorders other than a prepend, through both reposition and update_item;
- a `match` or `if` arm inside a row (an ArmFrame);
- a component instance inside a row: does a component body's read of the row's `i`, passed as a prop, count?;
- slot or derive defaults evaluated in a row;
- `with`/LoadBound frames between the row and an inner read;
- a row inside a row inside a virtualized list;
- the host-side virtualized collection's rebinding of wrappers to keys, in case a wrapper re-used for a different key skips evaluation.
Also: does anything outside runner/src/instance call reposition or update_item, or read frame.index, and assume a moved row was re-evaluated?
Checked: Excord on the iOS simulator, one history fling of 3 pages. Before: 3,147 evaluations, 1,107 of them unchanged. After: 1,966, with 19 unchanged.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll start from the round-1 brief and the two-commit diff, then try to construct a Contract program that would make a reorder disagree with a fresh build.The brief is the position-tracking change: a moved row stays all-dirty only when its body reads its own index. Next I’ll read both commits and hunt for a reorder whose output diverges from a fresh build.The logic change is small: `positions` tracks `LoadIndex`, and a move is all-dirty only when bit 0 is set. I’ll trace how that bit is computed through arms, components, `with`, and nested rows.The break attempt is already reasoned through. I’ll confirm the load-bearing call sites and line numbers, then write the round-3 findings.No P1, P2, or P3 findings.

**VERDICT: CLEAN**

A kept row is all-dirty on any index change exactly when its body reads that row’s position. Every `LoadIndex` the compiler can place in a row body lands on `bodies[region]` bit 0, and both reorder paths use that bit the same way. The programs below still match a fresh build of the new list. The cases that already differed from a fresh build (a slot initializer that captured `i`, a row held past the window) did so when every move was all-dirty.

### Reorders other than a prepend

Both gates compare the row’s old index with its new one. A move to the end, a swap, a sort, and a filter that drops earlier rows all take the same branch as a prepend.

- Virtualized window, same key: `Collection::reposition` (`runner/src/instance/collection/mod.rs:1009`) sets `moved` at `1016` and returns `!0` when `moved && reads_position` (`1018`). The window loop passes `body.reads_position()` and, on a data update, feeds that mask to `update_row` (`884–886`).
- Plain `for`: index-key (`region.rs:147`), same-order (`208`), and reuse-by-key (`265`) all call `update_item` (`303`). The mask is `!0` when `r.frame.index != frame.index && body.reads_position()` (`313`). Reuse-by-key also sets `roots` when the row’s list position changed (`264`), so child order is emitted even when the body is left as it was.

### `match` / `if` arm inside a row

An arm is a region frame. `Scope::lookup` counts only region frames (`contract/types/src/lib.rs:493–494`), and the lowerer pushes one: `push_none` for `if` and for `match`’s `none` arm (`contract/lower/src/lib.rs:1060`, `1110`), `push_some_of` for `match`’s `some` arm (`1098`). The arm’s nodes are parented to that arm (`1063`, `1083`, `1101`). A read of the row’s `i` inside the arm is `LoadIndex(1)`.

`Deps` stores the arm body unshifted, then shifts it by 1 into the region and unions that region into the `for` body (`runner/src/instance/deps.rs:357–363`). Bit 1 becomes bit 0, so `reads_position` (`165–166`) is set. `scan` sets `positions` and `frames` together on `LoadIndex` (`503–507`). With the row all-dirty, `enter` (`instance.rs:580`) marks that frame dirty for the arm, and `changed_outside` (`561`) re-evaluates a read of an outer frame even when the arm’s own bound is unchanged. A second nested arm is depth 2 and two shifts, still bit 0.

`out_of` (`176–186`) keeps bit 63 as “that far or farther.” Sixty-three single shifts still set bit 0. Wider masks only re-evaluate a row the old all-dirty path also re-evaluated.

An expression-level `match` or `let` does not push a VM frame. `push_let` is a non-region frame (`contract/types/src/frames.rs:38–39`), so the `LoadIndex` depth is unchanged and the opcode sits in the binding `scan` sees.

### Component instance, prop is `i`

It counts. Inlining replaces the component view with the use’s argument expressions (`contract/syntax/src/inline.rs:1–3`). A prop that is the row’s `i` lowers to `LoadIndex` in the use-site tree (`contract/lower/src/expr.rs:228–229`), inside whatever `if` / `for` / `match` the component body adds, and those region shifts fold it back to the row’s bit 0. A child `derive` read twice is an `Expr::Let` wrapped around that same expression (`contract/syntax/src/inline/derives.rs:261–268`), still in the scanned binding. Passing `i` through two components inlines to the same `LoadIndex`.

### Slot and derive defaults

A `derive` of that prop is substituted at each read (`inline.rs:77`) and follows the path above.

A `state` initializer runs in `init_owned` when the row or arm is created (`region.rs:426–435`) and again in `rebind` (`reuse.rs:308–320`). A later move leaves the slot as it was. All-dirty evaluation never re-ran initializers either, so a slot that captured `i` already survived a reorder with its old value (the per-key state rule). A fresh build shows the new index; that split predates this change.

### `with` / `LoadBound` between the row and the read

The view form is `match`. The `some` arm’s binding is `LoadBound` on an `ArmFrame`; the subject is compiled in the enclosing scope and copied into the region unshifted (`deps.rs:360`). A subject that is `i` sets the row’s position bit directly. A read of `i` inside the arm is the shift described above. There is no view-level `with`.

### A row inside a row inside a virtualized list

An inner `for` whose text is the outer index is `LoadIndex(1)` on the inner body. The outer body unions that region at shift 0 after the inner region has already shifted its body by 1, so the outer `reads_position` is set. `reposition` then all-dirties the outer row. The inner collection runs because `update_data` treats the body as changed from one scope further out (`collection/mod.rs:540`, `stale_outside(..., 1)`). An inner row whose own index and item are unchanged still re-evaluates: `into_scope` shifts the outer dirty bit onto the frame the inner `LoadIndex(1)` names (`deps.rs:191–192`, `instance.rs:561`). An inner row that reads only its own `j` leaves the outer bit clear, and its own move sets the inner bit 0 through `update_item` or the inner `reposition`.

### Wrapper reused for a different key

`Collection::rebind` (`reuse.rs:287`) builds a new frame at the new index, re-inits the row’s slots, forces a full pass, and calls `update_row` with `!0` (`332–337`). It does not consult `reads_position`. `views::rekey` (`views.rs:112`) is called only from there (`reuse.rs:341`). A same-key move is the window path above. `publish_position` (`views.rs:133`) runs from settle on every mounted row, separate from the dirty mask. The Windows IME `reposition` is a different function.

### Callers outside `runner/src/instance`

`reposition` and `update_item` have no callers outside this module. `Frame.index` is compared in `update_item` (`region.rs:313`) and `reposition` (`mod.rs:1016`), written there (`1017`) and on the temporary key-eval frame in `rekey.rs` (`62`, `170`). The only read outside `runner/src/instance` is `LoadIndex` in the VM (`vm.rs:793`), which runs when a binding is evaluated and sees the index already stored on the frame. A handler argument is evaluated at dispatch against those live frames, so a click sees the row’s current index after a move that skipped the row’s text bindings.
