# Forest — 99 Nights in the Forest, greybox

A clone of the Roblox game aimed at the engine's limits: world scale, light and
atmosphere, many agents. The findings are in
[the diary](../../diaries/005-forest.md).

```sh
bun game/dev.mjs forest                       # play
bun game/prove.mjs forest                     # verify against pins.json
bun game/games/forest/proof.mjs web           # pixels: artifacts/web/day.png, night.png
bun game/app/shells.mjs game/games/forest --test
```

## Playing

WASD moves. **E** chops the tree in reach (three blows; it drops two logs), picks up
a log, scrap or food (five at most, stacked on your back), feeds the fire when you
stand at it, or takes a lost child by the hand. **Q** eats. **F** toggles the
flashlight. Days are 80 s and nights 50 s.

The fire burns its fuel; its light and the safe radius shrink with it (4 m plus
0.2 m per fuel point). At night the Deer comes out at the edge of the light. It
will not enter the light; outside it, it closes in and charges. Hold the flashlight
on it for 0.6 s to stun it. Wolves roam in packs and bite outside the light, and
they scatter from the flashlight. Hunger drains; starving costs health, and the
fire heals you and recharges the flashlight. Bring both children into the light.
The HUD counts the nights you survive.

The title chooses the size of the forest (1k–250k trees; the world grows to keep
the density), the wolf count, extra torch lights, Rapier or grid collision against
trunks, and generated pines or primitive trees. These are canvas arguments, so a
choice rebuilds the world.

## How it is built

- `forest.rs`: one tree at most per 5 m cell, placed by selection sampling. The
  `Grove` resource keeps per-cell positions and hit points, so collision, steering
  and chopping visit the 3×3 cells around a point instead of every tree. Trees are
  instances of one generated pine model, or a trunk and crown of primitives.
- `camp.rs`: the cycle, the sky (`Environment` is written only when it changes),
  the sun moving into a dim blue moon, and the fire's point light. Point
  intensities are authored in the sun's units (`CANDELA`): the renderer passes
  `PointLight.intensity` through unscaled.
- `creatures.rs`: the Deer and the wolves are state machines over grid steering.
  The flashlight is a cone test against the player's facing; there is no spotlight,
  so its light is a point light pushed ahead of the player.
- `player.rs`: Rapier's `CapsuleController` against trunk colliders, or the same
  grid push-out the creatures use (`lite`), plus needs, carrying and children.

`logic/examples/scale.rs` and `bench.mjs` are the measuring tools behind the
diary's tables: hostless tick, save and restore costs, and live frame costs in
headless Chrome.
