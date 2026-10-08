# Issue #300 — mounted line-clamp first pixels

Issue: https://github.com/ccheever/exact2/issues/300

Base: `fa965d3e2b36417a22228900678f7abc68e72270`.
Implementation: `d185e65fbb87abac56a0c557b80fd111c59801ad`. Exact committed contents matched the passing source fingerprint in a separate detached checkout (`committed-source-comparison.json`).
Environment: macOS 26.6.2 (25G83), Apple Silicon, 420×360-point viewport at 2× backing scale; Swift 6.4, Rust 1.97.0, Bun 1.4.2.

`app.contract` is the exact issue fixture. `app.ts` supplies the current scaffold's required appId/grants/answer exports with no data sources. Created outside the framework with `bun scripts/exact.mjs new /tmp/exact-issue-300-repro`; framework source lives in the dedicated `fix-300-mounted-clamp` worktree.

## Actual window pixels

- `before-window.png`: after mounting/swapping via `tap t`, rows 2–4 lack ellipses. `before-capture.png` at the same state redraws those rows and hides the bug. `before-restyle-window.png` fixes the original build's pixels only after a color-scheme update.
- `after-window.png`: immediately after `tap t`, all rows show their trailing ellipses. `after-capture.png` agrees. `after-remount-window.png` repeats collapse after expanding again.
- `before-initial-window.png` / `after-initial-window.png`: startup controls and original wrapping text.
- `after-expanded-window.png`: toggle back to unclamped content.
- `web-reference.png`: Chrome reference after the same tap.

The window screenshots are actual composited window pixels (`screenshot … window`), including the title bar. Default captures omit the title bar. These are original PNGs, not regenerated or edited images.

## Reproduction commands

Run from the fixture directory with Bun 1.4.2 on PATH. Build each revision with `bun exact.mjs mac`. Each agent command launches a fresh isolated app instance.

```sh
bun exact.mjs agent macos --size 420x360 \
  "screenshot before-initial-window.png window" "tap t" tree "layout mounted" \
  "screenshot before-window.png window" "screenshot before-capture.png" \
  "prefer prefers-color-scheme dark" "screenshot before-restyle-window.png window"

bun exact.mjs agent macos --size 420x360 \
  "screenshot after-initial-window.png window" "tap t" tree "layout mounted" \
  "screenshot after-window.png window" "screenshot after-capture.png" \
  "tap t" "screenshot after-expanded-window.png window" \
  "tap t" "screenshot after-remount-window.png window"

bun exact.mjs agent web --size 420x360 "tap t" "screenshot web-reference.png"
```

`before-drive.log` and `after-drive.log` preserve replies, including `layout mounted` at 300×20 in both builds. Build logs record successful native bakes.

`pixel-comparison.txt` verifies that each after-window text row exactly matches the corresponding default-capture row (64-pixel title-bar offset). Before/after comparison changes exactly 60 pixels in each repaired row; at-launch and nowrap rows are byte-identical.

## Regression and validation

`test-before.log` runs the new regression against the original implementation: all six assertions fail for cached first-pixel lines (clamps 1/2 × left/center/right), while existing worker-versus-layout assertions pass.

`checks.json` is the exact verification argv recipe; `verification-bun142/report.json` records source fingerprints and all final results, with per-check stdout/stderr. The initial `verification/report.json` preserves failures caused by Cargo subprocesses finding Bun 1.3.14 on PATH; the final rerun prepends Bun 1.4.2 for every subprocess. The invocation is `PATH=/Users/daehyeonmun/.bun-1.4.2/bin:$PATH python3 /Users/daehyeonmun/.agents/skills/verify/scripts/run_checks.py checks.json --project-root <worktree> --output verification-bun142`. It runs the five repository gates (including both clippy and format) and `TextPaintTests|TextMetricsTests`.

Unchanged scheduling, cache residency policy, and unclamped/text-overflow paths are covered by source review plus existing text tests. No separate main-thread wall-time benchmark was performed; the required last line is now clamped on first paint, while other lines retain their cached typesetter path. iOS, tvOS, Linux and RTL clamp behavior were not separately exercised.
