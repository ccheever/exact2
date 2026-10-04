# Code review: named colours, document links in a capture, the source map in a web root (82b76214e..63790c0e3), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec` (codex-cli 0.157.1), reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `63790c0e3`.
- **Method:** one brief (sha256 `c415b0df418b91bd7921e0ae5dd5729dc35f3d734e3362fc78e35a110774cf15`), the same one sent to grok; round 1; blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message (`-o`), unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** all three fixed in `c2937e08c`: the table moved to exact-motion so keyframes take names (#3); both captures skip a link only when it is a .md/.txt document outside the static trees, so a linked script or an asset link is refused again (#1, #2), verified on a fresh app.

---

LAND WITH FIXES

1. **P2 — Skipped JavaScript links can still enter the web build.** [host/web-js/build.mjs:163](/tmp/rv-fixes/host/web-js/build.mjs:163) now skips `helper.js -> ../outside.js`. A regular `helper.d.ts` satisfies the staged TypeScript check, but the generated module imports the **original** `app.ts` ([line 256](/tmp/rv-fixes/host/web-js/build.mjs:256)), and Rolldown bundles that live graph without a capture-boundary check ([line 325](/tmp/rv-fixes/host/web-js/build.mjs:325)). Thus an imported link that previously caused refusal can supply external executable source absent from the checked snapshot. The native bake omits that `.js` file and fails to bundle it. **Fix:** bundle from the checked capture, or enforce capture membership and reject linked dependencies in the bundler’s load hook.

2. **P2 — Links inside asset trees now produce successful builds with missing assets.** The same predicate at [host/web-js/build.mjs:163](/tmp/rv-fixes/host/web-js/build.mjs:163) skips `assets/logo.png` when it is a symlink. The later recursive copy preserves it ([line 447](/tmp/rv-fixes/host/web-js/build.mjs:447)), while the web server rejects files whose resolved path differs ([host/web/serve.mjs:598](/tmp/rv-fixes/host/web/serve.mjs:598)); the image consequently returns 404. Rust correctly refuses every link under its captured asset trees ([js/bake/src/lib.rs:574](/tmp/rv-fixes/js/bake/src/lib.rs:574)), creating another host mismatch. **Fix:** retain refusal for all links within static trees and use the existing safe static-tree copier.

3. **P2 — Named colours remain invalid in keyframes.** [kernel/src/style.rs:749](/tmp/rv-fixes/kernel/src/style.rs:749) admits names, but [motion/src/color.rs:40](/tmp/rv-fixes/motion/src/color.rs:40) still requires `rgb()`/`rgba()` after its hex and transparent cases. Keyframe lowering passes authored strings directly to this parser ([contract/lower/src/svg.rs:418](/tmp/rv-fixes/contract/lower/src/svg.rs:418)). Consequently, `color="gray"` works on a node but fails with `lower-keyframes` inside a keyframe; named colours inside keyframed `light-dark()` and shadows fail too. **Fix:** share the named-colour lookup with motion through a dependency below the kernel, and cover these keyframe forms.

The table has 148 unique, sorted names and matches all existing canvas RGB values. Ordinary dynamic colours reach browser CSS or the kernel parser; SVG paint, gradients and ordinary `light-dark()` use the updated parser.

No additional deployment blocker found: the generated root map is excluded from new publications, production JS builds remove it, and the wasm publication allowlist excludes it. Development-server map access is intentional. Validation used read-only inspection and in-memory checks; no builds or filesystem-writing tests were run.