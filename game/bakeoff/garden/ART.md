# The art kit

`art/` holds every model as binary glTF 2.0 (`.glb`), metres, +Y up, facing −Z where a
facing matters. Each `.glb` references its textures by relative URI under
`art/textures/` (bark, cloth, grass, soil, stone, straw, wood — PNG). Every lane gets
the same bytes. Use them as given; do not regenerate or replace them. Primitives you
add yourself (the tile outline, rain, the sky) are fine.

| name | what | notes |
|---|---|---|
| `plant-<crop>-<0..4>` | a crop at growth stage 0–4 | origin at the soil, base ~0.85 m across. 14 crops: carrot strawberry blueberry tomato corn watermelon pumpkin apple bamboo coconut cactus dragon mango grape |
| `plant-<crop>-<n>-far` | the same, fewer triangles | optional level of detail beyond ~30 m |
| `fruit-<crop>-unripe`, `fruit-<crop>` | a fruit before and after ripening | origin at the fruit's centre. Material 0 (`body`) is the fruit's colour: recolour it per mutation (Gold `#f5c518`, Rainbow cycling hue, Frozen pale cyan, Wet/Chilled darker and bluer, Shocked yellow-white) |
| `fruit-…-far` | the same, fewer triangles | optional |
| `farmer-body` | the gardener: torso, head, straw hat | origin ~0.9 m above the ground (put it at y = 0.9 when standing) |
| `farmer-arm` ×2 | an arm, pivot at the shoulder | children of the body at x = ±0.38, y = 0.18; swing about x |
| `farmer-leg` ×2 | a leg, pivot at the hip | children of the body at x = ±0.18, y = −0.28; swing about x |
| `keeper-body`, `keeper-arm`, `keeper-leg` | the stall keeper | same joints as the farmer |
| `stall` | the seed stall | 4.7 m wide, origin at its floor |
| `barrel` | the blue water barrel | origin at its centre, 1 m tall: place at y = 0.5 |
| `can` | the watering can | shown in the gardener's hand while watering |
| `fence-post`, `fence-rail` | the picket fence | a rail spans 2 m (one tile) |
| `lantern` | a fence lantern | material 0 (`glass`) glows at night |
| `meadow` | the ground plane, 600 m square | origin at y = 0 |
| `grass-0`, `grass-1` | 10 m patches of grass blades | scatter around the garden |
| `tuft-0` … `tuft-3` | flower tufts | scatter along the garden's south edge and the meadow |
| `path-0` … `path-2` | stepping stones | a path from the stall to the garden |

The trees outside the fence are the grown `plant-apple-4`, `plant-mango-4` and
`plant-coconut-4` models, scaled up 2–3×.

The committed kit contains 294 models and seven textures, including optional
`golden-*` and `storybook-*` variants. These are the exact inputs copied into
run 1; the table above describes the models required by its brief. Source:
`7716302242:game/games/garden/art/`. The brief's blue barrel wording is a known
series-1 mismatch: `barrel.glb` is a wooden cask (LLP 1046.010 §6).
