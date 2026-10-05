# Forest — 99 Nights in the Forest, greybox

A clone of the Roblox game aimed at the engine's limits: world scale, light and
atmosphere, many agents. The findings are in
[the diary](../../diaries/005-forest.md).

```sh
bun game/dev.mjs forest                       # play
bun game/prove.mjs forest                     # verify against pins.json
bun game/games/forest/proof.mjs web           # pixels: artifacts/web/day.png, night.png
bun game/games/forest/proof.mjs web --playtest # Jev decisions; needs AI_GATEWAY_API_KEY
bun game/app/shells.mjs game/games/forest --test
```

## Playing

WASD moves. **Hold E** to chop the tree in reach (three blows; it drops two logs).
The prompt shows the remaining hits and the axe's 350 ms recovery between swings.
**Tap E** to pick up a log, scrap or food (five at most, stacked on your back),
feed the fire when you stand at it, or take a lost child by the hand. Holding
the axe does not collect the logs after the tree falls. **Q** eats. **F** toggles
the flashlight. Days are 80 s and nights 50 s.

The fire burns its fuel; its light and the safe radius shrink with it (4 m plus
0.2 m per fuel point). At night the Deer comes out at the edge of the light. It
will not enter the light; outside it, it closes in and charges. Hold the flashlight
on it for 0.6 s to stun it. Wolves roam in packs and bite outside the light, and
they scatter from the flashlight. Hunger drains; starving costs health, and the
fire heals you and recharges the flashlight. Bring both children into the light.
The HUD counts the nights you survive. Follow the rescue compass (W north, D east,
S south, A west) to the closest lost child; after **E** takes their hand it points
home. Each child reaching the lit fire brings **20 fuel and two food** at camp.
The reward is saved with the child's rescue and cannot be collected twice.

The compass buttons choose **Children**, **Fuel**, **Food** or **Camp**. Fuel
points to an uncollected log or scrap, then to a standing tree when none remain.
Food points to an uncollected meal. The chosen landmark stays fixed while you
walk and survives saving; collecting it selects another. A full pack containing
fuel points home. The campfire's bearing is always visible, and newly supplied
food refreshes an empty search. Logs add 12 fuel, scrap 20, and food 35 hunger.
The nights-survived score stops when you die.

Build a **camp windbreak** with **R** or its HUD button while standing by the
fire with **two logs and one scrap**. It permanently halves the fire's fuel
consumption in day and night, and appears as a metal screen on two wooden posts.
The materials are consumed instead of being burned for immediate fuel; extra
items stay in your pack. **Find logs** and **Find scrap** select those materials
on the same saved compass. The build button shows readiness, and the supply
budget includes the lower burn rate once it is built. Saving preserves the
upgrade, materials and its appearance.

The camp plan budgets supplies through the next dawn, including ten fuel and
ten hunger in reserve. Fuel must be in the fire; carried food counts if you eat
it. It names what is missing, reminds you to eat at 35 hunger or below, and tells
prepared campers to shelter near the fire until the displayed dawn countdown
ends. At dawn it budgets the next night. This guidance leaves your chosen
compass target alone, so you can still gather extra supplies or explore.
Camp readiness is independent of rescue progress: a prepared camp can shelter
you while children remain lost. The Children compass still guides their rescue,
and the HUD keeps their count and supply reward visible.

Add `--survival` to the Jev playtest to attempt two nights after rescuing both
children, with a 128-decision limit. It reads the same HUD, uses the compass
buttons and follows their bearings with normal keys. Cardinal detours are offered
after two strides fail to reduce the visible distance. Chopping uses a one-second
E hold; a recovery prompt offers its displayed wait instead of an unusable press.
Outcomes remain exploratory;
reaching the decision limit does not prove the survival goal was completed.
The final outcome includes a named Player read and position for diagnostics,
after the decision loop; these hidden observations are never supplied to Jev.

`--playtest --build-camp` is a separate 96-decision scenario: build the
windbreak and reach the first dawn alive. Jev reads its visible recipe and
build status, chooses the same material compasses, and can press the enabled
build button. After building, its wait action advances ten seconds. This is
an exploratory feature playtest; it leaves the earlier survival goal and
decision limit unchanged.

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
  the sun fading out under a second, unshadowed `DirectionalLight` (the moon), and
  the fire: a shadow-casting `PointLight` whose range follows the fuel.
- `creatures.rs`: the Deer and the wolves are state machines over grid steering.
  The flashlight's effect is a cone test against the player's facing; its light is
  a shadow-casting `SpotLight` from the player's chest.
  The Deer's `Visible` is written only when it changes: a mutable borrow alone
  makes the renderer rebuild every batch (it cost 100+ ms a frame at 100k trees).
- `player.rs`: Rapier's `CapsuleController` against trunk colliders, or the same
  grid push-out the creatures use (`lite`), plus needs, carrying and children.

`logic/examples/scale.rs` and `bench.mjs` are the measuring tools behind the
diary's tables: hostless tick, save and restore costs, and live frame costs in
headless Chrome.
