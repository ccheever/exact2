# Folding the newer engine line — 2026-09-20

Parents: land `8b290b2`, engine `64b5af1` (fetched from `/tmp/peers7.bundle`).
Prior engine parent: `8db5e42`. No remote service commands or agents used.

## Decisions

The engine is the default winner. Its renderer-owned fixtures, private
`register_audio` (public `sounds([..])`), game identity in pins, bake-time
`Game::NAME`/`Args::FIELDS`, generated-shell locked/offline policy, Linux
`gpu-dev` proof profile, authored manifest resolution under `.shells`, and
ureq 3.4.2 locks are retained. Engine Glow/gameplay changes remain; hashes will
be reconciled exclusively through `prove --repin` with all three modes agreeing.

Land's EXSIM v7, capture, typed scene, authored merge and transactional host work
remain. The restore implementation lives in `sim/checkpoint.rs`; incoming v5
restore code is integrated there rather than reintroducing a second codec.
`Game::register(&mut World, &Self::Args)` declares argument-dependent types in a
scratch registry. The entire saved world, hierarchy, exact tick/clock and input
validate before any setup. Setup constructs one candidate; authored three-way
merging follows decode. Both typed passes consume the existing shared load budget.
Standalone `from_save`/replay have no live registry and require declarations in
`register`. The fixed consumer dispatches registration through the same variant
branch as setup/tick. Capture fixtures retain every malformed-input assertion.

The renderer depends on no game. The fixed I3 CPU timing, clip negative control
and real primitive-feed assertions move to the fixed game's integration tests.
`exact_game_render::fixture::Recording::feed(&mut self, &mut Feed, &World)` is a
small game-independent recording consumer of the existing private `Writes` path;
no backend trait or game dependency is exposed. Its slot refusal remains 1,000,000.
The strict Fox residency test remains unignored with every assertion and real
adapter requirement. Its exact baked model/texture bytes are now renderer-owned
fixtures; the original game's asset includes no longer leak into the renderer.
The older optional residency fixture uses the incoming synthetic skinned model.
The dependency fence also rejects `verification/` and benchmark-game dependencies.

Host dev scheduling keeps land's staged transactions and stale-input checks while
using engine profile parsing and locked/offline generated-shell builds. Both sides'
independent tests are retained. No source exceeds 1,500 lines; unsafe stays in
engine storage. The root workspace remains separate from game and its consumers.

## Work and unfavorable cases

Restore is bounded by existing EXSIM/world decode size/allocation limits and
projection admission: 1M slots, 256 types, 16M visits, 512 MiB projection work,
128 MiB serialized base; excess work explicitly refuses atomically. The two world
decodes are linear in saved data and share their budget; the scratch is dropped
before setup. Existing 200k interleaved/churn, combined hierarchy-cycle,
malformed typed data before setup, partial assets, negative clip/feed and strict
animated GPU residency assertions remain. No favorable-case-only replacement.

Validation and pin moves follow in working commits. Environmental comparison and
full folded verification have separate notes/commits, as requested.

Merge gate: the game workspace builds. After adding missing pre-setup type
registrations to the capture fixtures, all 443 engine tests pass (7 ignored).
The additional same-name argument-selected decoder test also passes. Explicit
variant declarations take priority over inherited live registrations while
duplicate declarations within a registry still refuse. Caps: 736 sources; boot:
2 JavaScript modules, 1 wasm reference, 88,723 JS bytes, 3,298-byte page.
The initial broad sweep exposed 18 rendering adapter failures (three newly
incoming cases, plus the 15 previous cases); no rendering assertion is weakened.

## Recorder-only pin reconciliation

All seven commands `bun game/prove.mjs <game> --repin --hosts linux --reason ...`
completed, each after continuous, Save and FreshGame agreed on the entire tick
and save inventories. Receipts name Linux only. No web agreement is claimed.
The following are the only changed values relative to `8b290b2`:

| Game / key | Old | New | Reason |
|---|---|---|---|
| beacons / ticks 0 | `0x0d24c948c995ab14` | `0xb6e90714710c57ff` | Incoming engine Glow/gameplay state; world state |
| beacons / ticks 90 | `0x4f198d7b34cba602` | `0x164306fd0014d2eb` | Incoming engine Glow/gameplay state; world state |
| beacons / saves continuation | `8959bf658432452fac9bc4297f6053c1bcba8f455ee6fe3ce5de33eb11bdc478` | `53478fc1142c418fdb52961bfb365a8796eba8662bdf3466e34712cbaf08ffde` | Incoming engine Glow/gameplay state; EXSIM v7 continuation |
| greybox / ticks 0 | `0x9a871d8582d905e7` | `0xf560b8cf6dd9601d` | Incoming engine Glow/gameplay state; world state |
| greybox / ticks 90 | `0x71f43e51a13cc49f` | `0xc92812672686717e` | Incoming engine Glow/gameplay state; world state |
| greybox / saves continuation | `efd556700b72639a941e4d4f3575bcb9f01bc28fae8ba55e10a19dd14ff61843` | `4a14675296f891ef23732a91a1a9698e028ccd4f57159dbe6505e975d483ada4` | Incoming engine Glow/gameplay state; EXSIM v7 continuation |

Asset, particle, placement, skinned and sprite fixtures preserve all hash/save
values. Their repins update provenance only. Live/fixed Lanterns remain empty
until first Linux/web agreement; no manual baseline was published.

## External identity and scene-bake seams

Both Lanterns consumers have directory basename `lanterns`, but fixed I3 declares
`Game::ID = "lanterns-evidence"`. Publishing the basename would label its first
baseline incorrectly. State and wildcard snapshots now expose the actual game ID;
proof summaries retain it, and `agreePins` refuses missing or disagreeing IDs
across modes/hosts or against an existing baseline. Pin publication uses that
agreed identity. Five focused proof tests pass, including missing/foreign-ID
negative controls. This metadata changes no save or world hash. The full
Greybox state snapshot gains this field without deleting its existing assertions.

Scene baking now reads resolved manifest defaults and shares the generated-shell
Cargo policy. External proof tests create their fixtures under TMPDIR, preserving
their external-path assertions without writing sibling lane directories.

First full sweep: game 704 passed / 18 no-adapter failures / 23 ignored; fixed
consumer 4 passed / 1 ignored, including both moved CPU tests and the three-mode
I3 invariant. App compilation exposed an uninferred empty `sounds([])` name
type; both Lanterns declarations now use `sounds::<&str>([])`. Greybox's
unchanged incoming tree snapshot omitted the new Glow component; the snapshot
was updated from the actual failed reply, preserving all original entities and
fields. Its hash remains exclusively in pins.json. Clippy's Copy warning in
registry inheritance is corrected without changing behavior.
