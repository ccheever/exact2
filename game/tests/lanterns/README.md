# Fixed twelve-lantern I3 evidence

This is a separate app-owned test consumer, adapted from
`360921d386f212965cbc28229f21c93d280a753a:game/tests/lanterns`.
Gameplay changes belong in `game/games/lanterns`. The fixed source preserves the
measured twelve-lantern scene, ordinary-input script, apex and supplementary
moving-crate cases. Four independently compiled edits exercise placement,
gravity/jump, animation selection and appearance. `variant` selects those compiled
implementations only for this fixture; all share one save identity.

The integration test retains the full control/queued-input byte comparisons,
repeated checkpoint byte equality, all 120-tick per-edit field/collision/timer/
animation/report assertions and negative controls. It compares every checkpoint
and continuation artifact across continuous, Save and FreshGame. The host proof
loads those ordinary-input checkpoints through production Carry, runs all five
builds for both moments, and compares every complete save with that invariant-
checked oracle. It records 1,210 tick keys and 1,210 save keys through the existing
G recorder, e.g. `placement/apex/226`. Checkpoints retain intentional held input.
The viewport (1280×720) and integer-millisecond host clock are identical on the
CPU oracle and carriers; queue phase and viewport bytes are never discarded.

```
bun game/app/shells.mjs game/tests/lanterns
cargo test --manifest-path game/tests/lanterns/.shells/Cargo.toml -p lanterns-evidence-logic
bun game/prove.mjs game/tests/lanterns --hosts linux,web
```

The empty first baseline is published only after all three modes agree on Linux
and web, without `--repin`. Later changes use that command with `--repin`.
Detailed trajectories and saves are derived files under ignored `artifacts/`.
Each probe is bounded to 120 continued ticks; ten probes make 1,210 observations.
The production engine's admission limits apply to each scene, queue and restore.

No old save is migrated. Original EXSIM/EXGAME literals, the 605-line continuation
record and 612-entry binding migration inventory remain at the immutable incoming
ref. This combined-version evidence has a distinct game identity and EXSIM v7.
Schema/API mapping: `Character` → `CapsuleController`, old GLB declarations →
baked `fox.model` and digest, animation `seconds` → G `time`/explicit animation
step, queued input → the delivered-aware v7 record. The old Rust construction vs
scene test still compares all names, handles, parents, global transforms, hash and
now complete world bytes. Old binary before-0/60/180 assertions are historical
receipts, not a compatibility promise for the combined runtime.

The apex has an airborne controller and active spring, but the crate has already
slept. Nonzero crate velocity and spin are tested at the supplementary checkpoint.
There is no animation blend coverage. CPU timing reports full simulation including
animation and primitive feed separately; it makes no GPU or palette timing claim.
