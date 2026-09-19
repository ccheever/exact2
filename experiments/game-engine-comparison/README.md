# Game-engine comparison evidence — 2026-09-17

See [LLP 1041.003](../../llp/1041.003-game-engines-for-agents.research.md)
for the recommendation, method and limitations. These are diagnostic
microfixtures, outside the five blocking checks. No production dependency
was added to Exact. Repetition counts are runtime executions, not agent trials.

Each directory retains the actual authored source and JSON from its fleet
lane. Reports identify machine, versions, setup, sources and limitations.
`baseline*` files preserve the pre-change fixture. Final files implement
the subsequently disclosed release/repress and in-memory restore task.
JSON output timestamps use UTC (September 18); the session date is
September 17 Pacific. Do not overwrite the retained results when rerunning:
copy a lane to a scratch directory first.

## Browser lanes

The original machines still have their isolated installations at
`/tmp/exact-engine-comparison-20260917/browser3d` on `expose-builder-pixel`
and `.../browser2d` on `expose-builder-pixel-a`. Dependencies are pinned
in each `bun.lock`; fleet Bun was 1.3.14. In a scratch copy:

```sh
bun install --frozen-lockfile
```

The runners name the existing fleet Chromium executable explicitly:
`/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome`.
Use that same executable for reproduction on the original hosts, or
adjust the path in a scratch copy for another installed browser and
record the difference. Browser launch flags select headless/software
rendering. Servers and browsers close after the proof.

From `browser3d`, run each final proof separately:

```sh
bun h1-run.mjs three
bun h1-run.mjs babylon
bun h1-run.mjs playcanvas
```

`negative-run.mjs three` loads the separate intentionally broken
`negative-fixture.js` and should exit 1. `run.mjs` is the original
baseline; inspect its JSON flags because that historical driver did not
aggregate them into exit status. `sizes.mjs` retains the separate bundle
experiment. `FINDINGS.md` explains why its output is not a production-size
or frame-performance claim.

From `browser2d`, `bun probe.mjs` runs both final proofs and exits nonzero
on failure. The retained negative JSON comes from intentionally bypassing
the fourth-lantern gate in scratch fixture copies; the final HTML files
have the correct gate restored. `baseline/` preserves the original
sources, results and hashes. `REPORT.md` identifies the unequal input,
clock and rendering paths.

## Native lane

The original `expose-builder-nothing` scratch directory is
`/tmp/exact-engine-comparison-20260917/native`. Binaries and package
downloads remain there; they are not committed. `versions.json` records
versions and binary hashes. Run from that provisioned directory:

```sh
./Godot_v4.7.2-stable_linux.x86_64 --headless --path godot --script main.gd
./squashfs-root/AppRun love
PYTHONPATH=pydeps python3 pygame/main.py
```

Each prints its proof and exits nonzero on failure. The `*-baseline`
directories preserve the pre-change sources. `runs.json` and
`heldout-runs.json` retain five process repetitions per working engine;
`negative-control.json` retains the failure controls. Godot's earlier
assertion timeout remains in `negative-control-first.json` rather than
being hidden. Defold source and `defold-build.json` retain the last failed
build; no Defold runtime result exists. Reprovisioning requires the
recorded upstream versions, Pygame 2.6.1 in an isolated Python target,
and for Defold the toolchain/library dependencies described in `REPORT.md`.

## Unity

Only the official CLI was downloaded and run. `unity/` retains version,
binary SHA256, empty installed-editor inventory and the no-connected-
Pipeline-instance result. There is no Unity game fixture or successful
engine proof. Account login, license provisioning and vendor agent
plugins were not exercised in any lane.

## Repository validation

`caps`, `boot`, `cargo fmt --all -- --check`, and staged whitespace checks
passed. Workspace build was interrupted after more than five minutes
without completing; the queued workspace test and clippy commands were
waiting for its build-directory lock and were interrupted too. These are
not recorded as passing. Only this task's command sessions were stopped.
Local Cargo commands used `EXACT_UPDATE_TRUST=development`.
