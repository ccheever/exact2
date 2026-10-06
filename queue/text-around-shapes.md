**Text around shapes** (LLP 1043.000 §8, as built 2026-09-19): `wrap-flow` / `shape-outside` exclusions flowed on
both sides per frame on Linux, Apple and web through one shared walker (`textflow/`), demo `apps/textflow` (six
scenes); vendored Taffy is upstream 0.14 (patches 3, 4, 5 retained). Owed: **auto-height flow** — M8's probe found
`BlockContext` offsets are provisional at measure time (sibling margin collapse y=100 → 90, auto margins x=0 → 50,
descendant collapse y=110 → 150), so stable offsets and wrapping-context identity must be settled before measurement
sees them, or bounded re-layout used instead (Charlie's call; LLP 1043.000 §8 "Stage 0"); the wasm cost is ~+77 KiB against the ruled ~64 KB;
Euclidean `shape-margin` for ellipse/polygon; `justify` in fragments; iOS hit-testing and any iOS run at all;
Safari/Firefox; `shape-outside: <image>`.

*Filed under “Next, in order (2026-08-29)”.*
