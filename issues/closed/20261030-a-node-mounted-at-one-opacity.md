# A node mounted at one opacity keeps it on Apple hosts after it changes

**Status:** Closed
**Resolution:** fixed: a batch that presented a property of a view it creates presents that property again when it returns to identity (`Batch::at_identity`)
**Systems:** host/apple (`src/batch.rs`, `src/presence.rs`)
**Author:** Daehyeon Mun
**Date:** 2026-10-30

**Problem.** In the T3 Code clone (Settings › Devices › EAS Simulator), a saved host's row mounts while the
save runs, so its Edit and Remove mount disabled at `opacity` 0.64. The kernel's opacity then returns to 1,
but the Mac draws them at 0.64 until the page mounts them again. The cause is in the Rust host that the
Mac, iOS and tvOS share, so iOS and tvOS had it too.

**Cause.** One batch presented twice. The commit that mounts the row presents `opacity 0.64` for each
button. The batch then runs a `resize` round before it is finished (`resize_rounds`, `src/resize.rs`): the
round first runs the timers due by then, and the clone's save ended in one of them, so `busy` went false
in the same batch. The round's presentation found each button's opacity at 1, its identity, and left it
out, because `present_value` (`src/presence.rs`) assumed the presenter starts a view this batch creates
at identity. This batch had already moved it to 0.64. The batch the presenter applied:

```
create id=1249 (button, disabled=true)
present opacity id=1249 x=0.64
props id=1249 disabled=false
```

Nothing presented it again, since the engine's value no longer changed. A probe in the Mac presenter
recorded that sequence in the clone at `b9e83df3a5^`; the kernel's computed opacity was 1 and the view's
alpha 0.64.

**Fix.** `Batch` records the properties it presents of each view it creates, and `present_value` skips an
identity value only for a view the batch creates and has not yet presented that property of
(`Batch::at_identity`).

**Acceptance.** `mount_opacity::a_row_mounted_dimmed_and_lifted_by_a_resize_round_is_presented_whole`
(`host/apple/tests/it`) and `MountOpacityMacTests.testAButtonMountedDimmedAndLiftedInTheSameBatchIsDrawnWhole`
fail before the fix and pass after it; the clone at `b9e83df3a5^` draws a freshly saved host's Edit and
Remove at alpha 1.
