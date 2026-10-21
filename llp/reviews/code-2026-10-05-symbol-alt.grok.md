# Code review: symbols decorative unless named (exact-web test, JS target, LLP 1007/1011/1035.004), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless; round 1 at `c9f247102`, round 2 at `cb36c9e28`.
- **Method:** the shared briefs (sha256 `735d11c0903cda0814e8f7bb80f316d2c2b8a49c86da90dd9911d91d6db0b3bb`, `3e404592b522d8d5ef13c4230f1124669c0e74f11c9c27b995fa3aa2f4e09002`); blind. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; opening sentences are progress narration.
- **Verdict:** round 1 LAND WITH FIXES; round 2 LAND.
- **Disposition (round 1), taken in `2c5ea3d5b`:** LLP 1011 §3 is the web rule; §2 and 1035.004 D1 say a named symbol keeps its name on the web and macOS and that iOS still hides symbols, without claiming iOS. Noted, not changed (both predate the commit): a cleared bound label leaves no `alt`, and `unsymbol` leaves a stale `alt=""`.

---

## Round 1

I'll review only commit `bdff631bd` against its parent, read-only, and start by reading the review instructions and the diff.The diff is small. Next I’ll check whether `c9e5b9d17` really made named symbols keep their name, and whether the JS change matches that on every source path.LAND WITH FIXES

Reading `c9e5b9d17` as intended is right. Its message and the `element.rs` comment say a symbol no longer overwrites an authored name with `alt=""`. The old test was stale. Restoring "always decorative" would undo that. The `symbols.js` change matches it: a static label is already on the element (template `alt` from `props_of`; images never get `aria-label`), and a dynamic `src` is registered before `accessibilityLabel`, so a later label write still wins. macOS agrees (`updateRoleAccessibility`: a labelled image is an element). iOS does not, and that only matters for the spec sentence below.

1. **P2** — `llp/1011-image-v1.spec.md:132-134` (and the unedited `:155-156`). §2 now states a cross-host rule, but §3 still says the web host supplies `alt=""` for every symbol, and iOS still drops the name: `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:449` sets `isAccessibilityElement = false` on every symbol. Linux has no image accessible-name path. `llp/1035.004-native-visual-affordances.rfc.md:129-131` still says a symbol carries no label. Edit §3 to the web rule (decorative `alt=""` unless `accessibilityLabel`, written as `alt`). Keep §2 decorative, and say the exception is the web host and macOS. Point the 2026-10-04 line at 1035.004 D1 as a web amendment. Do not claim iOS.

The JS unit test only checks the hook with `alt` already set. It does not cover a label cleared while `src` stays a symbol (`P` removes `alt` and the hook does not put `alt=""` back) or `alt=""` left behind when `src` leaves a symbol for `app:/` or a plain image (`unsymbol` does not touch `alt`). Both predate this commit. The wasm test does check the two static cases this commit is about.

## Round 2

I'll review only commits `bdff631bd` and `2c5ea3d5b` against the symbol-alt behavior from `c9e5b9d17`, and report what is still wrong.The two commits are in. Next I’ll read the symbol path, the `c9e5b9d17` change, and the host/docs they claim to match.**LAND**

No findings.

`c9e5b9d17` is the intended rule: its message says a symbol no longer overwrites an authored name with `alt=""`. Restoring "always decorative" would undo that. `props_of` already writes `accessibilityLabel` as `alt` and inserts `alt=""` only when that key is absent (`host/web/src/element.rs`).

`symbols.js` matches it. The hook runs only for a `symbol:` source, after `h()` has applied the template attributes, and `P("alt")` owns later label changes. A non-empty `alt` is kept across a source change (symbol, `app:/`, or a plain URL). An empty or missing `alt` becomes `alt=""`. A bound label is a separate effect: setting or changing it writes `alt`; clearing it to `""` is decorative. Static named and unnamed symbols get their `alt` from the shared template, not from this hook.

Native hosts were already split, and this commit does not need to move them. macOS exposes a labelled image, symbol included (`updateRoleAccessibility`). iOS forces `isAccessibilityElement = false` in `updateSymbol`. Linux never reads `accessibilityLabel`. The LLP 1011 / 1035.004 notes match that.

The host test locks both alts on the wasm render; the JS test locks the hook's keep-versus-set. `host.rs` stays at 1,499 lines. The spec edits describe the code that ships.
