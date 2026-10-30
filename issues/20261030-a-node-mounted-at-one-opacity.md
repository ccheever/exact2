# A node mounted at one opacity keeps it on the Mac after it changes

**Status:** Open
**Systems:** host/apple
**Author:** Daehyeon Mun
**Date:** 2026-10-30

**Problem.** In the T3 Code clone (Settings › Devices › EAS Simulator), a saved host's row mounts while the
save runs, so its Edit and Remove mount disabled at `opacity` 0.64. The next commit makes the kernel's
opacity 1, but the Mac draws them at 0.64 until the page mounts them again.

**Cause.** Not found. The host sends `present opacity 1` in the next batch (`host/apple/tests/it/mount_opacity.rs`),
and the presenter draws it, with the save in a modal dialog and an `EasGhostButton`-like button
(`MountOpacityMacTests`). The other writers of a NodeView's alpha are the disabled look
(`Mac/PressLookMac.swift:113`) and a held press source (`Mac/MouseChainMac.swift:673`).

**Reproduction.** The clone before `b9e83df3a5` (`opacity=(disabled ? 0.64 : 1)`): save a new EAS host.

**Fix.** Record the clone's batches (`agent.mjs trace --pid`), find the op that is lost or written again,
and make the fixtures above reproduce it.

**Acceptance.** A fixture fails before the fix and passes after it; the clone's saved row draws at full ink.
