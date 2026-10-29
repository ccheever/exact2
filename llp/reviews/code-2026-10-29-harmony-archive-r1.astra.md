# Code review: Harmony production archive (development-only grant lines), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d4988514a7`.
- **Method:** one brief (sha256 `f33259212f4f72aaa70225cf5422ab219eb78421ab70c033a24cab860cd59322`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [bake/src/reach.rs:40](bake/src/reach.rs:40): **The rejection breaks existing local-network development bakes.** The TypeScript producer already adds `development` to `net.fetch local-network` and `net.websocket local-network` ([lib.rs:1188](js/bake/src/lib.rs:1188)). Those valid lines now trigger `grant-development`, including Lights’ grants. Distinguish the new author annotation from the existing loopback modifier, and cover the complete bake path.

2. [js/bake/src/typescript.mjs:972](js/bake/src/typescript.mjs:972): **A web-JS production build can retain development grants.** The direct `host/web-js/build.mjs … --production --plan …` path sets its production flag but never sets `EXACT_UPDATE_TRUST` ([build.mjs:61](host/web-js/build.mjs:61)). With trust unset, `configure` keeps marked grants while the builder emits a production bundle and copies the production bake’s metadata. Resolve trust consistently at that entrypoint and reject disagreement with the supplied bake. Apple archive/release and deploy callers already propagate production trust correctly.

3. [js/bake/src/typescript.mjs:219](js/bake/src/typescript.mjs:219): **Trailing whitespace bypasses the rewrite.** `device.microphone purpose.microphone development ` and lines joined with `'
'` retain the marker. Native compatibility generation subsequently rejects them; the web-JS normalizer preserves device lines, and runtime admission checks their first token ([modules.js:802](host/web-js/modules.js:802)). Thus a marked device grant can remain effective under production trust. Recognize the final token after trailing whitespace while preserving unmarked lines; cover spaces, tabs and CRLF.

4. [js/bake/src/typescript.mjs:219](js/bake/src/typescript.mjs:219), [bake/src/reach.rs:40](bake/src/reach.rs:40): **A required argument named `development` is mistaken for an annotation.** Valid grants such as `secret.keep development` or `device.microphone development` are deleted in production, stripped into invalid grants in development, and rejected unchanged from Rust. Recognize an additional marker after a complete grant, rather than any final word with that spelling.

5. [js/bake/src/typescript.mjs:972](js/bake/src/typescript.mjs:972): **Marked plain network grants stop following backend selection.** For `net.fetch http://127.0.0.1:4400 development`, `injectBackendOrigin`’s exact comparison ([line 196](js/bake/src/typescript.mjs:196)) misses the marked line. The subsequent rewrite removes the marker but leaves the old origin, while application requests use the selected backend and are refused. Preserve the suffix while rewriting network origins; test literal and `.join` forms with a backend override.

6. [js/bake/src/typescript.mjs:222](js/bake/src/typescript.mjs:222): **Export aliases accepted by `staticFacts` crash the rewrite.** `const permissions = 'device.hid development'; export { permissions as grants };` produces a grants value, but the declaration lookup finds no identifier named `grants`; `declaration.init` then throws. Resolve the exported binding using the same export mapping as `staticFacts`, and rewrite that initializer.

**Non-blocking**

1. [scripts/harmony-production.test.mjs:57](scripts/harmony-production.test.mjs:57): The third test establishes false availability and an absent row, but does not establish that these are the only failures. Additional failures within that same test still pass its assertions. A failed `testCallBuild` source can also produce the empty fallback and matching diagnostics. Prefer explicit production assertions plus `expect no failures`; check subprocess status, signal and the complete result set. It also does not exercise the inert `testCallStart` guard.

2. [bake/src/reach.rs:40](bake/src/reach.rs:40): The promised Rust diagnostic is inconsistent. An ordinary I/O grant such as `fs.read app:/cache/test-call development` fails `reach::parsed` before reaching this check ([compat.rs:213](bake/src/compat.rs:213)). Validate Rust annotations before ordinary parsing and test through the public compatibility entrypoint; the new test calls `derive` directly.

3. [docs/contract-for-agents.md:1193](docs/contract-for-agents.md:1193): Qualify `--release` as Apple’s flag. Android’s flag selects an optimization profile without forcing production trust, and plain Cargo release builds also default to development trust. Document `EXACT_UPDATE_TRUST=production` as the governing setting.

Static review only; no files edited, network accessed, tests or builds run.
