# bench — the same scenes here, in Godot and in three.js

Diagnostic, never a check (LLP 1041.000 §6a). `bun game/bench/run.mjs <engine> <scene>
<n> [mode] [variant]` prints one JSON line. Every engine presents to the display at
2560×1440 pixels, 4× MSAA, vsync on, so the honest question is **the largest N that
still holds the refresh rate**, with the script and render milliseconds beside it.
A run records the machine's load; this machine is shared, so compare runs taken together.

Each twin is given its best idiomatic effort and its best optimized one — a strawman
twin measures nothing. Here there is one mode: you spawn entities.

## Scenes

**`cubes`** — N unit cubes on a cubic grid (side ⌈∛N⌉, spacing 2, centred), cube *i*
turning about axis normalize(frac(i·0.3719)−½, frac(i·0.7331)−½, frac(i·0.1913)−½) at
0.5 + (i mod 7)·0.25 rad/s, coloured by hue frac(i·0.61803). One directional light, no
shadows, flat ambient. A 60° camera orbits at radius 1.8·side + 6, height 0.35·radius,
period 20 s. It measures what it costs to move many things: per-entity update, the
transform path to the GPU, the draw.

Modes — Godot: `nodes` (a `MeshInstance3D` each), `multimesh` (one `MultiMesh`, set from
GDScript each frame), `shader` (the rotation in the vertex shader; no CPU work). three.js:
`meshes` (a `Mesh` each), `instanced` (one `InstancedMesh`, set from JS each frame).

## Baselines — 2026-09-17, M5 Max, macOS, 120 Hz, machine under other load

| engine | mode | N | fps | p95 ms | script ms |
|---|---|---|---|---|---|
| Godot 4.7.2 Forward+ | nodes | 2,000 | 119 | 9.3 | — |
| | nodes | 10,000 | 18 | 56.1 | — |
| | multimesh | 100,000 | 118 | 8.8 | 5.9 |
| | shader | 1,000,000 | 119 | 8.6 | 0 |
| three.js r186 WebGL | meshes | 10,000 | 57 | 19.7 | 2.3 |
| | instanced | 100,000 | 120 | 9.9 | 2.2 |
| | instanced | 200,000 | 120 | 9.9 | 4.1 |
| three.js r186 WebGPU | meshes | 10,000 | 20 | 58.8 | 2.3 |
| | instanced | 100,000 | 120 | 10.0 | 2.3 |

The bar these set: the idiomatic path here — one entity per cube — should hold 120 Hz
past where both twins' *optimized* paths stop.
