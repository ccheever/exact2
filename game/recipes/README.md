# Game recipes

Each recipe has a runnable `Sim` fragment, assertions and the pitfalls beside it.
Run them with `cargo test --manifest-path game/Cargo.toml -p exact-game --doc recipes`.
They are compiled directly from these pages by `engine/src/recipes.rs`.

| Task | Recipe | Existing game or reference |
|---|---|---|
| Follow a player | [Follow camera](follow-camera.md) | Starter |
| Find and take a pickup | [Pickup and prompt](pickup.md) | Beacons |
| Display remaining time | [Timer HUD](timer-hud.md) | Starter HUD |
| Turn a first-person camera | [Mouse look](mouse-look.md) | Engine look tests |
| Play on a landing edge | [Sound on contact](sound-on-contact.md) | Audio fixture |
| Spawn at an interval | [Spawner](spawner.md) | Particle fixture |
| Answer a command | [Door message](door-message.md) | Engine HUD and events |
| Draw a ring and a translucent halo | [Halo](halo.md) | Beacons |
| Save a slot | [Save slot](save-slot.md) | Starter restore tests |

Use the [engine reference](../engine/README.md) for borrowing and registration.
Names, units and defaults are available through `bun scripts/exact.mjs game vocab <name>`.
