# LLP 1022: The serial runtime owner, measured and parked

**Type:** Research
**Status:** Draft
**Systems:** Apple host (both presenters, `Bridge.swift`, the boot path), Kernel (`Env.keyboard`, the wire), Agent API (LLP 1012's determinism), Web host (the parity constraint)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Related:** LLP 1008 (the Apple host; a §11 "one runtime owner" exists only in this lane's parked doc edits), LLP 1012 (the eight operations and the clock), LLP 1016 D2 (async settlement — the wake pump this lane rehouses), LLP 1018 (the durable store the reload carries), LLP 1019 §11 (the fonts fold — independent evidence below), branch `parked/runtime-owner` (2583be3, on origin)

## Summary

A large uncommitted lane appeared in the main working tree on 2026-08-30: `ExactRuntime`
— one dedicated thread owning the runner and kernel, with UIKit/AppKit never entering
Rust and complete batches published back to main — together with the kernel half of
env()/keyboard, the Contract and web/linux host halves, and presenter field work. Its
author was never identified (six sessions answered a roll call; all cleared themselves).
Before landing it we measured it, and the measurements said no: it fails the macOS
smoke most runs, changes what "boot" means, and buys a modest iOS startup win that a
much smaller change could buy alone. Charlie ruled 2026-08-30: park it, write this,
return later if the system's evolution demands it. The code is whole on
`parked/runtime-owner`; the main tree is clean of it.

## What the lane is

`ExactRuntime` (`Bridge.swift`): every C call — events, resize/insets/keyboard,
tick/advance, the wake pump, boot — is captured on main, enqueued FIFO to a dedicated
`exact.runtime` thread, executed there, and its complete batch published to main as a
closure. Boot becomes asynchronous behind `finishBoot`; iOS starts `prepare()` before
`UIApplicationMain`; a `motionPending` gate stops display-link ticks from stacking;
insets+keyboard+size travel as one viewport publication; `barrier()` lets the agent
wait for every submitted result's apply; the dev-loop reboot's `presenter.reset()`
moves inside the publication so the old tree survives until the new plan's first
batch. The same files carry the kernel's `Env.keyboard`/`Dimension::Keyboard` and
presenter field work (placeholder, password-reveal keep-focus, keyboard animation),
which the Apple half depends on — the lane does not decompose along file lines.

## Findings

All measurements 2026-08-30, on detached-worktree builds of exactly the states named,
base `17350d0` (pre-fonts). The baseline-worktree mechanic: `git worktree add --detach`
plus `cp -al` of an existing cargo target dir (12 s, no extra disk).

1. **It fails its own gate.** The lane's exact tree failed `scripts/smoke.mjs macos`
   ~6 of 8 runs — the motion seek across the timer lands 75,50 where 75,100 is
   expected, and canvas captures over-count — while the base passed 6 of 6. The
   asynchrony leaks past LLP 1012's settle guarantees somewhere in the clock or
   canvas-settle path; the smoke is the repro.

2. **macOS startup regresses, and "boot" changes meaning.** Synchronous base: boot
   ~127 ms mean (112–137, 6 runs), first frame complete at 275 views. Lane: boot
   ~153 ms mean (143–160) for an **11-view shell**, content settling in afterward;
   painted ~198 vs ~204 ms — a wash, and the lane's "painted" may be painting the
   shell. Any future comparison across this lane must measure time-to-settled-frame,
   not the boot printout.

3. **iOS startup improves modestly — and the mechanism is separable.** Base ~389 ms
   mean (371–406, 3 runs); lane ~362 ms (358–366), ranges non-overlapping, full 275
   views both. The win comes from `prepare()` running before `UIApplicationMain`:
   UIKit's ~300 ms of scene setup gives preparation and settlement time to finish.
   That overlap is a one-shot async — it does not need the permanent owner thread.

4. **The boot path punishes reshaping — independent evidence.** The fonts lane
   (LLP 1019 §11) merely added a parameter to `exact_boot`/`exact_boot_plan` and
   broke macOS canvas capture outright — 0 captures, 99.80% readback delta — caught
   only by a baseline-worktree comparison; the diff touched no canvas or GPU code.

5. **The structural costs are permanent, not teething.** The wasm host runs the
   kernel synchronously on the browser's main thread and always will (workers would
   fork the eight-op semantics), so the parity oracle is a synchronous oracle. And
   the agent's contract — the call returned, therefore it settled — becomes a
   discipline every future Apple-host feature must re-earn through `barrier()`
   ordering. The 6-of-8 flake is the first invoice of that tax, not a one-off bug.

6. **The problem it solves has not arrived.** Runner + layout is ~43 ms at boot and
   ~0.5 ms in steady state; scrolling is native and never enters the kernel. No
   profiled interaction hitch attributable to kernel-on-main exists in any app today.

## Confidence

The smoke failure rates are solid (8 lane runs, 6 base runs, two distinct failure
modes, reproduced across two worktrees). The startup deltas are medium: 3–6 runs per
flavor on one machine, ±20 ms run-to-run noise, iOS on the simulator only — the iOS
ranges not overlapping is the strongest of these signals. The shell-frame mechanism
(async settlement racing the window) is inferred from the view counts and the
prepare/present split, not traced; the seek race's exact location is unknown.

## What to take without the owner

Four of the lane's ideas stand alone in the synchronous world and are worth
cherry-picking when wanted: the `motionPending` tick gate; insets+keyboard+size as
one viewport transaction; `presenter.reset()` after the new plan's first batch on
reload; and the one-shot pre-`UIApplicationMain` prepare overlap on iOS (finding 3).
A fifth is scoped, not general: when LLP 1016's data settlement needs threads, give
the **executor replies** an off-main preparation path — data is already outside the
deterministic-clock contract, so the responsiveness win lands where determinism
isn't at stake.

## Conditions for return

Revive the lane when a profiled hitch on a device traces to kernel work on the main
thread — a real app, a real interaction, a number — or when plan/text scale makes
boot-on-main visibly block. A revival must: rebase `parked/runtime-owner` across
43b0c0c (fonts touch `Bridge.swift`, `Text.swift`, `exact.h`, both presenters — and
`exact_set_fonts` now runs inside boot, so reload re-registers the catalog); adopt
time-to-settled-frame as the startup metric; and treat the smoke's seek and capture
invariants as the acceptance test, verified by baseline-worktree A/B before any push.

## Where the code is

Branch `parked/runtime-owner` at 2583be3 (pushed to origin), one commit on base
17350d0, blob-for-blob the main worktree's state at parking time: the Swift and Rust
across kernel/contract/web/linux/apple, the lane's LLP doc edits (1000, 1001, 1007,
1008, 1012, 1016, 1018 — including the §11 that names the owner), and the
runtime-flavored dev-menu port that rode with it. Authorship remains unknown;
whoever recognizes the lane should claim the branch.

## Open questions

- Where exactly does the seek race live — the agent clock path, or canvas settle?
  (The smoke is the repro; nobody has traced it.)
- Who wrote the lane? Provenance matters for reviving its intent, not just its text.
