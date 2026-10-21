# Round 1
I'll review the seven newest commits on HEAD without changing anything, starting with the review instructions and the commit range.The request is a read-only review of the seven newest commits, focused on the kernel registration change and whether the test updates still assert the original behavior. I'll read those commits and the surrounding layout rules next.The diffs are small. Next I’ll read the region validator, the layout rules it claims to follow, and each test it touches.The region walk is the part that can refuse a legal tree. I'll check its bound against the per-branch limit, then whether each test still asserts the original condition.The static-owner check is safe for the layout agreement, and the six test edits still assert the original conditions. One registration check refuses regions the size limit allows.

## Findings

[P2] The absolute-descendant walk applies one node cap to both branches and refuses a legal static region — `kernel/src/region/state.rs:680`

`members` allows `REGION_NODES` (4096) mounted nodes in the content branch and another 4096 in the pending branch, and each trial is built from one branch. `holds_absolute` stacks both of the owner's children and returns `true` once `seen > REGION_NODES`. `validate` turns that into the structural error "requires one attached independent clipped owner and two direct branches". A static owner with no absolute box and, for example, 3000 nodes in each branch is inside both limits, and the trial and the ordinary tree place those nodes the same way, but registration and every later commit fail. The comment above the check says `members` would refuse that tree. It would not.

## Agreement

An absolute box's containing block is its nearest ancestor that is not `static`, or the top of the tree (`kernel/src/layout/hoist.rs:8`). A trial makes the owner that top, so a static owner contains an absolute that nothing between them contains. That is the only layout disagreement this gate is there for.

Sticky does not add another one. It is lowered to relative for layout and for that containing-block walk, and the scroll offset is applied by the host after layout. Fixed is not a position value. The new check is stricter than that rule on purpose: any absolute under a static owner is refused, including one already contained by a relative, absolute, or sticky box in between. That refuses some regions whose two layouts would agree. It does not admit a region whose layouts disagree.

A descendant that becomes absolute later is seen. `validate` runs at registration, on every commit (`observe`, `kernel/src/region/state.rs:124`), and on every region layout (`kernel/src/region/state.rs:230`). A failure drops the candidate and makes the next `compute_region_layout` return an error. The region stays registered and ordinary layout stays gated, which is the same refusal an owner that becomes `height: auto` already gets.

The walk runs only for a static owner. It is one depth-first pass over the two branches, on commits that already walk those branches in `members`, and it stops after 4097 visits. That cost is small. The cap's accounting is the defect above: one child list is still copied whole before the next check, as in `members`.

## Tests

The other six edits still check the condition they were written for.

- Unknown event kinds are still refused, and the state is unchanged. Kind 29 is pointerdown (`f95bdf401`); 43 is past the last accepted kind, 42.
- An eager reorder list is still refused, with the same error id the Linux test asserts, `lower-reorder-collection`. The runtime "no hold" checks are unreachable once that source does not compile. The comment cites `425f97dab`, which only edits the LLP write-up. The refusal was added in `3f8cbb165`, which is what the Linux comment says after its review fix.
- `developmentLaunchEnvironment` is the same function, imported from `host/apple/devices.mjs`, where `28a2ee81d` exported it.
- The inset test still expects an empty batch from an app that does not read `env()`. Caltrain's sky does, since `cf992c8ee`. The plain view does not, and `set_env` returns false when nothing does.
- The box-shadow assertion is the same string. `25e70c67f` had already reduced the loop to that one row, and the string includes the closing bracket, so a second shadow would still fail it.
- The deadline test still requires the 300 ms and 400 ms timeout failures, a 700 ms lower bound, and both sockets to close. The upper bounds moved to 30 s and 45 s, under the servers' new 60 s hold, so a client that never cancels still fails.

The new region test still refuses a static owner over the absolute placeholder, and it allows that owner once the placeholder is static. It does not compare that region's frames with an ordinary layout, and it does not add an absolute after registration.

# Round 2
I'll review those two commits as they stand, without changing any files. First I'll read the review instructions and the diffs.This is a read-only review of those two commits. I'll inspect the diffs and the surrounding call sites, then check the specific claims about the region walk and the bake-time deferral.Both commits are in this tree. I'll read the region walk and the bake-deferral path around the diffs, including the cases the counter used to cover.Auto-compacting conversation (81% full)...
Conversation compacted.
The kernel walk looks sound; I'm finishing the bake-deferral check on the cases the removed counter used to cover.Every resource type has a zero, so a missing `else` is not a hole by itself. Next I’ll check mutations, `parse_for` after a bake-time fetch, and anything the counter’s removal left behind.No findings.

**854c3b132.** `holds_absolute` gives each direct child of the owner its own stack and `seen`, and `validate` has already required exactly those two branches. Children are copied only when `seen + stack.len() + children.len() <= REGION_NODES` (`kernel/src/region/state.rs:682`), so a branch past the cap is treated as holding an absolute and nothing past the cap is copied. A static owner with an absolute descendant still fails `validate`, which runs at registration (`RegionState::new`), on `observe`, and on every `compute`.

**218bec42d.** `DeferredAtBake` covers the cases the counter actually deferred. The counter ran only in `answer_for`, only for `Target::Resource`, and only on `Unavailable` after door op 5 had refused storage while baking. An uncaught refusal still leaves the prelude with `code: 'bake'`, and `step` maps that to `DeferredAtBake` while `baking` is set. Settlement then shows the `else` row or the type's zero (every `TypeKind`, including records), marks the row pending, and enqueues nothing. Mutations never matched the counter. `parse_for` never did either, and bake does not fulfill a fetch, so a fetch-then-storage source stays pending the same way. Worker validation is inline on the inspect module, so the error returns to that same settlement path; a later worker instance is a fresh module with `baking` clear. The web check still does not run the module. `bake_refusals` and `UNASKED` are gone, with no remaining uses.

The content-region test only flips the fixture's direct placeholder. A nested absolute and a branch at the node cap are covered by the walk, not by that test.
