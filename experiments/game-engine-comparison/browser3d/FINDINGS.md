# Browser 3D comparison evidence, 2026-09-17

Recommendation: track **PlayCanvas** and **Babylon.js** as the browser 3D contenders. PlayCanvas has the stronger documented complete agent workflow: first-party code skills and an Editor MCP able to edit scene/project data and inspect runtime entities, logs, screenshots and injected input. Babylon has richer documented headless/native avenues and official graph-authoring MCP plus an experimental Inspector CLI. Keep Three.js as the renderer/reference baseline; this experiment does not show an engine winner. These are tracking recommendations, not adoption or Exact scope expansion.

## Measured scope

Fleet: ccheever@100.65.137.99, hostname expose-builder-pixel; Linux 6.8.0-138-generic; AMD EPYC 9454 48 cores/96 threads. Bun 1.3.14, Chromium 151.0.7922.34 via cached Playwright browser, software SwiftShader. Packages pinned in package.json and bun.lock: three 0.186.0, @babylonjs/core 9.27.0, playcanvas 2.22.2, playwright 1.63.0. All installs isolated under /tmp/exact-engine-comparison-20260917/browser3d; no global installs, repo edits or fresh clones. Dependency installation of all four packages together: 1.28 seconds wall (setup.log). Existing browser cache excluded.

The fixture creates each engine's actual plane/player/three lantern meshes/entities and camera; drives application-owned movement by sixty explicit ticks, mutates engine transforms and visibility/enabled state, collects lanterns by application distance test, then resets. Each trial opens a new browser page. All 15 baseline trials passed with no page exceptions. These are repeated executions of one agent-authored fixture, not 15 independent authoring tasks. They prove direct API automation and inspectability, not engine physics, deterministic engine clocks or gameplay quality.

| Engine | Baseline passes | Navigation→ready median ms | 60 tick+render submission median ms | H1 passes | H1 entire five-trial proof process s |
|---|---:|---:|---:|---:|---:|
| Three.js | 5/5 | 107.64 | 5.8 | 5/5 | 1.712 |
| Babylon.js | 5/5 | 1758.88 | 22.3 | 5/5 | 9.968 |
| PlayCanvas | 5/5 | 190.08 | 14.7 | 5/5 | 2.114 |

Baseline entire proof process: 12.707s, including Chromium launch, navigation, five fresh pages per engine, one screenshot per engine, and teardown. Ready includes unbundled module import/compilation, local server requests, fixture construction and CPU render submission; Babylon additionally waits for scene.whenReadyAsync. Its barrel import pulls a large module graph. **Do not compare these as production startup/first pixel.** Sixty render calls measure synchronous CPU submission overhead, not completed GPU frames, sustained FPS or frame latency. Memory unmeasured. Agent prompt-to-proof timing unmeasured; one shared authoring pass makes independent per-engine task timings misleading.

One baseline fix round: Babylon initially had valid object state but blank screenshot because shaders were not ready. Added whenReadyAsync, reran all trials and inspected screenshot. Same round removed harmless missing favicon server errors. This directly demonstrates why object assertions alone are insufficient. Baseline driver originally printed pass flags without failing exit status; H1 driver aggregates pass flags and returns nonzero on failures. Baseline result flags were all true.

Baseline freeze SHA256: fixture d620b42ffbadc24dc8a33e01be2c3a1df7e4fbe6daabfbe08a3f5020f0186d26; results fdcb6bdf29806d0acb65028cd70ba50a27e3a1dac16dbacd211186ad0d29156a; run 319d74c493c5cbd5db96c6ed6f82203fb68b1a9b539c521e61032ec52fb5336c.

## Held-out H1 and trust control

Parent disclosed H1 only after baseline frozen: add fourth lantern beyond endpoint, held action cannot collect it; release+repress must collect; save/reset/restore must recover all four collected and same player position. Changed fixture advances to tick80/playerX4 with action held: score3/fourth visible. Release and press yields score4/fourth hidden. JSON save, reset, restore compares complete inspected state. All engines passed5/5 on first implementation. Human interventions0. The baseline only had programmatic tick/reset; H1 adds an app-level boolean action(down) bridge. No actual keyboard or pointer event route was tested.

Negative control changed i<3 to i<4 so the fourth auto-collects while action is held. H1 scorer rejected all five Three.js executions and returned exit1; positive source restored. This tests the common scorer, not three independent controls.

Six successful screenshots captured and visually inspected: three baseline showing plane/player/three lanterns, and three H1 showing restored player at endpoint with lanterns absent. An initial unsuccessful Babylon screenshot was also inspected and led to the readiness fix. No image similarity or pixel assertions.

## Size probe

Same Bun.build minify:true,target:browser, default ESM, no splitting; each baseline adapter extracted into its own entry and imports its engine package. gzip is Node zlib default gzipSync. This is fixture-plus-package bundle output, not minimal carefully subpath-imported production builds and not engines' inherent minimum sizes. Babylon uses its broad barrel import. Two extraction correction rounds, then successful build; size probe output not separately browser-tested.

| Fixture | raw bytes | gzip bytes |
|---|---:|---:|
| Three.js | 751755 | 196544 |
| Babylon.js | 8225392 | 1882441 |
| PlayCanvas | 3002949 | 794623 |

## Primary-source research (checked 2026-09-17)

- Three.js Object3D exposes named hierarchy traversal and JSON serialization: https://threejs.org/docs/pages/Object3D.html . Scene/rendering library composition is straightforward for text editing and runtime inspection. Physics integration is separately chosen: official list https://threejs.org/manual/en/libraries-and-plugins and Ammo adapter https://threejs.org/docs/pages/AmmoPhysics.html . No official Three.js agent suite was established in this bounded search (not a claim none exists). CPU scene objects can be manipulated without drawing; rendering still needs an appropriate graphics environment. Our test used browser WebGL, not Node rendering or native packaging.
- Babylon official NullEngine explicitly supports server tests, simulation and loaders without producing pixels: https://github.com/BabylonJS/Documentation/blob/master/content/setup/support/serverSide.md . Fixed-time lockstep and before/after-step observables are documented: https://github.com/BabylonJS/Documentation/blob/master/content/features/featuresDeepDive/animation/advanced_animations.md . These were researched, not exercised.
- Babylon official @babylonjs/mcp-servers authors/validates/imports/exports/live-syncs material, geometry, render, particle, GUI, flow and smart-filter graphs: https://github.com/BabylonJS/Documentation/blob/master/content/toolsAndResources/mcpServers.md . Distinguish graph-authoring tools from gameplay input/state automation. Experimental Inspector CLI exposes scene entities/properties/stats and custom commands via StartInspectable: https://forum.babylonjs.com/t/inspector-cli-for-ai-agents/63243 . Neither tool exercised here.
- BabylonNative targets Windows/macOS/iOS/Android/Linux, but README labels it source-only public preview and lists feature gaps (e.g. audio/serializers/particles, partial GUI/input): https://github.com/BabylonJS/BabylonNative . Native builds untested. Worth tracking, not evidence of parity.
- PlayCanvas standalone code path avoids requiring Editor: https://developer.playcanvas.com/user-manual/engine/standalone/ . Its documented first-party AI skills target Engine/React/Web Components, not Editor services: https://developer.playcanvas.com/user-manual/getting-started/use-playcanvas-skills/ . Official Editor MCP is the complementary direct scene/project/runtime workflow: https://developer.playcanvas.com/user-manual/editor/mcp-server/ and https://github.com/playcanvas/editor-mcp-server . It requires an open logged-in Editor project and local server; only one Editor connected; no project administration. Docs include runtime entity query, logs, screenshot and keyboard/mouse/touch injection. Not exercised here.
- PlayCanvas public Application.update(dt) and render APIs permit explicit update/render separation: https://api.playcanvas.com/engine/classes/Application.html . Current physics system documents timeScale0 pause plus explicit step, fixedTimeStep/maxSubSteps: https://api.playcanvas.com/engine/classes/RigidBodyComponentSystem.html . Ammo/Bullet backend documented: https://developer.playcanvas.com/user-manual/physics/ . Null graphics backend exists in documented device selection: https://developer.playcanvas.com/user-manual/web-components/tags/pc-app/ . No physics/NullGraphicsDevice/native export measured.

Re-entry/watch triggers: PlayCanvas official tooling should reproduce held-input/restore behavior through actual runtime input and state without custom eval bridge; Babylon Inspector CLI should do same and native parity must be proven on required platforms. Three.js becomes an active engine candidate only if intentionally choosing to own gameplay/physics/state infrastructure, or if a substantive official agent integration changes that tradeoff.
