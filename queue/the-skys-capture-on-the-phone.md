**The sky's capture on the phone** landed at the display's rate 2026-08-30 (LLP 1008
§9): a shadow layer tree rendered by `CARenderer`, a nested canvas's readback cached,
the texture handed to the module as it is (`gpu_texture_metal`, `gpu_sync`) — 119 fps
on average scrolling with the sky on, a capture 7.1 ms, from 42–64 fps and 20–25 ms.
Left: the deck's 48 per-child textures still cross as bytes (a `gpu_child_metal`
would spare ~30 ms when the deck opens); the macOS presenter still captures on the
CPU (`cacheDisplay`, 5–18 ms at 2×) — the same `Shadow` would serve it; the
cross-queue wait (`gpu_sync`, ~3 ms of the 7.1) could be a shared Metal event if
wgpu exposed one. Tried and declined on the way: capture at 2× (slower), `CARenderer`
on the live overlay (crashes — a layer is in one tree only), `drawHierarchy` (no gain).

*Filed under “Cheap, any time”.*
