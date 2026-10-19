# Game recipes

Each recipe has a runnable `Sim` fragment, assertions and the pitfalls beside it.
Run them with `cargo test --manifest-path game/Cargo.toml -p exact-game --doc recipes`.
They are compiled directly from these pages by `engine/src/recipes.rs`.

| Task | Recipe | Existing game or reference |
|---|---|---|
| Spawn props and sprites | [Literal builders](literal-builders.md) | Builder state tests |
| Follow a player | [Follow camera](follow-camera.md) | Starter |
| Find and take a pickup | [Pickup and prompt](pickup.md) | Beacons |
| Display remaining time | [Timer HUD](timer-hud.md) | Starter HUD |
| Turn a first-person camera | [Mouse look](mouse-look.md) | Engine look tests |
| Play on a landing edge | [Sound on contact](sound-on-contact.md) | Audio fixture |
| Spawn at an interval | [Spawner](spawner.md) | Particle fixture |
| Answer a command | [Door message](door-message.md) | Engine HUD and events |
| Draw a ring and a translucent halo | [Halo](halo.md) | Beacons |
| Save a slot | [Save slot](save-slot.md) | Starter restore tests |
| Bubbles, a wake, a splash | [Emitter](emitter.md) | Particles fixture |
| Make sounds without files | [Synthesized sounds](synthesized-sounds.md) | Audio fixture |
| Orbit a third-person camera | [Orbit camera](orbit-camera.md) | Engine look tests |
| Flash what was hit | [Hit flash](hit-flash.md) | Engine presentation tests |
| Catch a tap inside one tick | [Same-tick press](same-tick-press.md) | Engine input tests |
| Export models from Blender | [Blender export](blender-export.md) | Asset fixture |
| Light a tank or a sea from inside | [Underwater](underwater.md) | Underwater GPU test |

Use the [engine reference](../engine/README.md) for borrowing and registration.
Names, units and defaults are available through `bun scripts/exact.mjs game vocab <name>`.
