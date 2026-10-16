# Code review: Swift and Kotlin bindings, and the sample host (Tamber stage 5, commit 5), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `b460f39bb5df58c1266226e53360a6cbd2b89de6e212afce1f26c9856ca54fbc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the CLI's output, unedited (its progress lines run into the first paragraph).
- **Verdict:** as transcribed below.
- **Disposition:** in the first-round commit.

---

I'll review HEAD read-only against the stage 5 brief and LLP 1121, starting with the commit, the brief, and the design.The request is a read-only defect review of HEAD, so I’ll inspect the commit, the stage 5 brief, and the D12 design without changing the worktree.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open. I'll confirm the cited lines in the bindings, the host hook, and the Android path, then write the P0–P3 report.Nothing here is P0. Two call patterns are broken; the ABI gates, the Kotlin wait split, and the smoke's stated checks are sound.

## P1

**A second `observe` on one library `ExactTamber` never runs.** `Tambers.moved` calls `place.inputs()` inside the `where` (`host/apple/modules/ExactTamber.swift:141-144`). `inputs()` writes the new inputs before returning (`:100-104`), and every `observe` on that value shares one `TamberPlace` (`:66-69`). `let t = ExactTamber(view); t.observe { a() }; t.observe { b() }` then `setMode`: `a` runs, `b` sees equal inputs and is skipped. Two separate `ExactTamber(view)` values are fine. The in-process path re-asks once, then notifies every observer (`host/apple/Sources/ExactKit/Tamber.swift:139-144`). The sample and the Mac test register one observer, so they miss this.

**An iOS/tvOS library dynamic colour resolved off the main thread is the fallback.** `hatchSession` returns nil off main (`NativeHatches.swift:73-74`), so `hatchTamber` returns 0 (`:160-165`) and the provider uses `.clear` (`ExactTamber.swift:41-57`). UIKit calls a `UIColor` dynamic provider on a background thread. `view.backgroundColor = ExactTamber(view).color(.surfaceCard)` then paints clear whenever that happens. macOS `NSColor` providers are main-thread. The in-process binding does not use this gate.

## P2

**Wide, profiled, and moving colours never reach a module.** `textChannels` appends a space code and four components (`Batch.swift:100-109`). The library keeps a side only when `count == 4` (`ExactTamber.swift:41`), so every side is the caller's fallback. In-process `TextEngine.color` accepts the nine. Separately, a side `textChannels` cannot resolve is sent as `[0,0,0,0]` (`NativeHatches.swift:168-170`), which is four channels, so the module paints transparent black instead of the fallback. System colours stay four channels; the eight-side index matches (bit 0 dark, bit 1 contrast, bit 2 elevated). macOS and tvOS both force elevated off, on both sides of the hatch.

**In-process `ExactTamber(view)` freezes the place at init** (`Tamber.swift:39-43`). Built before the view is under a `NodeView`, `session` is nil forever: every colour is the fallback and `observe` is not registered (`:99-100`). Reparenting keeps the old node id. The library and Kotlin walk again on each ask.

**Android's colour int ignores contrast.** `tamber_json` paints `colour.resolve(dark)` (`host/linux/src/tamber.rs:63-66`). `resolve` takes only `dark` (`kernel/src/style.rs:728`); `reported` is keyed `(kind, id, dark)` (`kernel/src/style/roles.rs:162`). `inputs` still hashes contrast (`runner/src/runner/tamber.rs:203`), so `observe` runs and `color()` returns the same ARGB. Increased Contrast on a role token does not change the int.

**A batch applied while the owner is waiting on that same main turn does not refresh.** `Runtime.tamber` uses `on(busy: nil)` (`Tamber.swift:161`). Main with `serving > 0` gets the busy answer (`Owner.swift:92-94`). In-process `moved` then `continue`s and keeps the old cache (`Tamber.swift:141-144`); the library hatch returns 0, so `inputs()` is false. A `setMode` inside a `callMain` the owner is blocked in never notifies. The smoke's `setMode` is not this path: owner slices land later via `DispatchQueue.main.async` (`Session.swift:626`).

**Kotlin `check` re-reads the cached node, and the frame is posted before that.** `check` asks `id`, not `place()` (`Tamber.kt:76-81`). Move a view onto a pinned node: the next `color()` re-walks and is right, but `observe` does not fire if the old node's inputs did not move, so a colour applied once stays on the old token. `Owner.produce` posts the frame (`Owner.kt:403`) and only then calls `Tambers.turned()` (`:407`). Main can draw `color()` from the previous cache for that frame. `ask` does not deadlock: it does not call `produce`, `produce`'s `main.post` does not wait, and `check` calls `Native.tamber` on the exact thread and posts observers (`Tamber.kt:86`). JNI uses UTF-16 (`jni.rs:1022-1030`, `field.rs:146-164`), not modified UTF-8.

## P3

**Every batch pays the hook.** `Session.apply` always calls `Tambers.moved` (`Session.swift:1058`). In-process, every cached place is re-asked even with zero observers (`Tamber.swift:140-143`); a dead view id whose answer is nil is kept and retried (`:141`). The library hook is one call, then one host round trip per observer (`ExactTamber.swift:143`). Registries hold weaks. They do not keep views or sessions alive. `made` / `bySession` drop dead entries on the next insert.

**The two-call hatch buffer fails closed** if the answer length changes between the length probe and the write (`ExactTamber.swift:111-121`). The view pointer is `Unmanaged.passUnretained` for the duration of that synchronous call only.

**Sample elevation.** The swatch stores `cgColor` (`apps/tamber-host/modules/apple/Swatch.swift:47`). iOS re-applies only when `hasDifferentColorAppearance` (`:68-70`), which does not include `userInterfaceLevel`. The sample's own light/dark hex tokens do not change with elevation. A module that snapshots a system or role colour the same way stays on the base side in an elevated popover. The reference already says to re-apply from `observe` because a layer keeps a `CGColor` (`docs/reference.md:1303-1305`).

**`NewString` is unchecked** (`field.rs:146-148`). An OOM returns a null `jstring` to `Native.tamber`.

**Parked Android view.** A row removed from `byView` resolves as the root (`Tamber.kt:57-64`, `Natives.kt:234-240`) until it is put back. `color()` on the next draw re-walks.

**macOS library colours are built with `NSColor(red:green:blue:alpha:)`, calibrated RGB** (`ExactTamber.swift:42`). In-process uses sRGB. The sample's hex round-trip is what the smoke checks; this is the residual.

## Tests

They test what they claim, and not the bugs above. `scripts/smoke-tamber.mjs` checks boot colours, `setMode` moving the window swatch only, both swatches following the dark scheme, and a root font size incrementing both counts. That is one library observer, a `cgColor` snapshot, on the main thread. `TamberBindingMacTests` is in-process AppKit: pin, fallback, one observer, `stop`. It re-reads `color()` after `setMode`. It does not keep a layer. The Linux integration test covers the JSON host (pins, scheme, missing name, bad view, live batch), not JNI or Kotlin. `contract tamber gen swift` is asserted as source text (`surfaceCard`, a backquoted `` `default` ``). Nothing covers a second observer, an off-main provider, a nine-channel colour, contrast, elevation, host-table size negotiation, or Kotlin.

ABI: host table is 104 bytes with `tamber` at 96 (`NativeHatches.swift:189-204`); the library reads it only when size ≥ 104 (`ExactNativeModule.swift:537-538`). The module entry is at 216 in a 224-byte table (`ExactNativeModule.swift:1437`); the host calls it only when size ≥ 224 (`NativeModule.swift:224`). An older host or module skips the entry. The accent guard is held for the whole Linux answer and restored on drop (`host/linux/src/tamber.rs:53`). The stage 5 brief file was not readable here; this is against D12, `docs/reference.md`, and HEAD.
