# Code review: the JS agent tree's inline image type, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `e9143679ed`.
- **Method:** one brief (sha256 `87037bfdef1e0781d0f2a4dc7466558a54f50f526f0991899c63f52c64b3b824`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The fix is right and complete. An inline `<img>` is the only non-text node a paragraph can hold, and this predicate types it `Image`, which is the runner's type. The image record matches the runner. The new test is a real JS-agent test and is red on the parent by inspection.

**Blocking**

none

**Non-blocking**

1. `host/web-js/document-agent.test.mjs:213` — the expect locks `type`, `accessibilityLabel`, and `imageSource`, and never locks the other field this change drops. `host/web-js/agent.js:101` sets `props.text` only for `Text`, `SvgText`, and `SvgTSpan`, so an `Image` omits it. The runner's tree does the same: `runner/src/agent/tree.rs:77` writes `node.node_type.name()` (`"Image"`, `kernel/tables/schema.json:15`) and `runner/src/agent.rs:445-465` writes only schema props, and an Image has no `Text`. Conform's norm treats a missing `text` and `""` as the same string (`host/web-js/conform.mjs:192`), so this gap would not show up as a type failure. Add `text: by('inline')?.props.text` to that expect, expected `undefined`.

An inline image is an `Image` whose parent is a non-run `Text` (`kernel/src/arena/inline_image.rs:9-13`). `NodeType::in_text` is only `Text | Image` (`kernel/src/node.rs:106-108`). The kernel rejects any other child of a `Text` (`kernel/src/txn.rs:427-433`), the runner refuses the same document (`runner/src/instance/document.rs:288-289`), and lowering allows a paragraph only `text` runs, `time` runs, and `image`s (`contract/lower/src/nodes.rs:802-805`, `844-851`). A video, a canvas, an audio element, an iframe, a button, or an SVG element, including `<symbol>`, cannot be a paragraph child. SVG elements are classified from their namespace before `run` (`host/web-js/agent.js:93`), and they can live only inside an SVG container (`kernel/src/txn.rs:435-444`). A `symbol:` image is still an `<img>` with `data-symbol-source` (`host/web/src/element.rs:1343-1351`), so `tagName !== 'IMG'` covers it and `props.imageSource` stays `dataset.symbolSource` (`host/web-js/agent.js:97`). A `<time>` run and an inline link stay `Text`: both are `NodeType::Text` emitted as `<time>` or `<a>` (`host/web/src/element.rs:488-495`), and `TYPES.A` is `Pressable` (`host/web-js/agent.js:70`) only when the run short-circuit does not fire. Extending the exception past `IMG` would type that link `Pressable`.

`&&` binds tighter than `||`, so the text arm is `option || data-exact-text || (run(el) && tagName !== 'IMG')` (`host/web-js/agent.js:93`). The paragraph carries `data-exact-text` (`host/web/src/element.rs:875-879`) and the image is emitted as its child (`host/web-js/src/emit.rs:1093-1095`, `1447`), so `run(img)` is true. On the parent that arm was `run(el) ? 'Text'`, which returns `Text` before `TYPES.IMG`. The new test opens the JS host (`host/web-js/document-agent.test.mjs:209`, `host/web-js/agent.js:348-360`), and its `type: 'Image'` expect fails there. The label, the source, the paragraph's `Text`, and the two word runs can pass on the parent, because those do not depend on `type`. The test was not executed; tests and builds were out of scope. The same `<img>` predicate covers all three images in `host/web-js/conformance/inline-image.contract`. That target was not re-run. Dropping `props.text` from `""` to absent does not add a conform field (`conform.mjs:192`), and `scripts/agent-compare.mjs:303` now keeps the image as a box candidate, as the wasm tree already did. The diff is two files, +42/−1. The plan named in the commit is not in this tree, so there was no written cap to check against.

The record matches. A `data:` source is copied as `src` (`host/web-js/src/rows.rs:986`; `package_scheme` excludes `data`, `rows.rs:1005-1007`) and `releaseAsset` leaves it unchanged (`host/web-js/elements.js:66-70`). The agent reads it with `getAttribute('src')` (`agent.js:97`). The label is `alt` (`host/web/src/element.rs:947-949`, `agent.js:98-99`). `props.text` is absent, as above.

Every reader that could depend on an inline image being `Text` is in the commit message and does not:

- `agent.js:101` — `props.text` is `Text` only. The image now has none, as on the runner. `scripts/agent-test.mjs:23-31` joins descendant `props.text`; an `<img>` contributes `""` either way, so the paragraph string is unchanged. The commit cites the Control, Pressable, and TextInput branches at lines 27-28, 89, 98, and 424, which do not see this node.
- `scripts/agent-compare.mjs:303` — comparison candidates leave out `Text`. The image is now a candidate, as on wasm. The commit says so. Labels at 229 and 236 only print the type.
- `scripts/agent-check.mjs:195`, `scripts/agent-inspect.mjs:165`, `:194`, `:247`, `:271`, `:398`, `scripts/agent.mjs:851`, and `host/web-js/inspect-dev.js:270` copy or print the type. Both sides of the layout check are now `Image`.
- `scripts/agent-drag.mjs:157` only looks for `ScrollView` and `List`.
- `scripts/agent-ax.mjs:134` is an accessibility name-source type. `host/web-js/check.js:260` is the DOM control's type. Neither reads this tree.
- `apps/design-tool` reads no tree type. `scripts/design-tool.test.mjs:160` keeps `type === 'Text'` to read dial labels; that page has no paragraph image.
- `scripts/stage12-named.mjs:77` filters layout `type === 'Image'` and is called only for `web-wasm` (`scripts/stage12-gate.mjs:211`), whose type already comes from the runner.
