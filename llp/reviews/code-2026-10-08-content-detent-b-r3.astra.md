- **Should-fix — synthetic segments stop working inside fit-content routes.** [env.rs:344](/private/tmp/bsky4-rv/wt-dvb/kernel/src/style/env.rs:344) selects the screen grid, but [Agent.swift:466](/private/tmp/bsky4-rv/wt-dvb/host/apple/Sources/ExactKit/Agent.swift:466) updates only the viewport grid. On a non-folding iPhone, `prefer segments 1x2` succeeds, yet `env(viewport-segment-height 0 0)` inside the route remains undefined. Propagate the override into both grids, using their respective dimensions, and test through the agent path.

- **Should-fix — round-2 sibling allocation remains unfixed.** [geometry.rs:223](/private/tmp/bsky4-rv/wt-dvb/kernel/src/kernel/geometry.rs:223) still gives the stand-in only the route. A 400-point flex row with a fixed 200-point sibling and a `flex=1 min-width=0` route measures that route at 400 instead of 200, underestimating wrapped content height. Preserve sibling constraints during inline allocation.

- **Should-fix — round-2 SVG/canvas resolution remains unfixed.** [length.rs:113](/private/tmp/bsky4-rv/wt-dvb/kernel/src/svg/length.rs:113) still resolves nested SVG `height="50vh"` to zero; [scene.rs:250](/private/tmp/bsky4-rv/wt-dvb/kernel/src/svg/scene.rs:250) drops viewport-unit padding from SVG/canvas drawing boxes. Resolve dimensions through `env_for` and use resolved layout padding.

By inspection, button faces, region certificates, parent-padding trials, stale comments, ordinary route/stack scoping, and reboot resend are corrected. No web/Linux/macOS regression found.

The new assertions would catch removal of their fixes: button padding becomes 20 instead of 84.4; segment height becomes 100 instead of 400; border-box parent padding changes wrapping. Execution was unavailable: redirected offline Cargo resolution failed on missing `windows-sys`. Region certification, reboot, and agent segment overrides lack new regression coverage.

Nothing rises to a landing blocker; the three should-fixes remain.