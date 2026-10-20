# Review: LLP 1126 r1 (system surfaces)

**Family:** GPT-6 Astra (OpenAI) · **Provider/runtime:** codex exec -m gpt-6-astra, model_reasoning_effort=max, --sandbox read-only · **Date:** 2026-10-17 · **Redacted:** no · **Method:** blind (no reviewer saw another's review), read-only checkout of origin/main 8bf68cf88 plus the r1 draft; the author's brief asked for high-level, directional and foundational findings only (prompt: the same text to each reviewer)
**Verdict:** READY WITH CHANGES

---

## Verdict

**READY WITH CHANGES.** The direction is right: retain CSS layout, emit semantic SwiftUI views, separate rendering from refresh work, and defer the live driver. Templates should remain separate. The foundational gaps concern **ownership across processes, compatibility across releases, and the extension runtime’s actual boundary**. These can be resolved without abandoning the proposed renderer. The five classes are useful descriptions, but execution model alone does not determine a renderer or packaging model.

This was a read-only checkout review; no builds or external verification were performed.

## Strengths

- **D2’s one-app authoring model is sound.** Shared types and one coordinated bake prevent unnecessary duplication. Logical unity should still permit different artifacts and capability sets for each entry.
- **D3, D6 and D7 establish the right boundaries.** Refresh work produces inputs; platform time elements remain live; interactions become durable commands or URLs.
- **D4’s semantic SwiftUI output is preferable to pictures.** Keeping CSS box layout while preserving platform text, accessibility and rendering roles fits [LLP 1115 §§2–4](/Users/ccheever/projects/exact2-wt-surfaces/llp/1115-write-the-web-ship-the-platform.principle.md:48).
- **Snapshot first, live later is sensible.** Reusing primitives is worthwhile without replacing the existing UIKit/AppKit presenters. The instance-scoped [Apple C ABI](/Users/ccheever/projects/exact2-wt-surfaces/host/apple/include/exact.h:277) provides a credible starting point.

## Foundational concerns

**HIGH — D8 needs a cross-process ownership model, not just a shared location.**

The existing update store holds an **exclusive lock for its owner’s lifetime**; another process cannot simply open it concurrently ([`update/src/store.rs`](/Users/ccheever/projects/exact2-wt-surfaces/update/src/store.rs:465)). Meanwhile, [LLP 1031 D4](/Users/ccheever/projects/exact2-wt-surfaces/llp/1031-brownfield-embedding.rfc.md:335) explicitly gives sessions private store snapshots: a peer’s writes are invisible until a later refresh boundary. Its all-session generation transaction is also confined to one process.

**Consequence:** moving files into an app group does not establish D8’s “one state” and “one plan.” Widget actions can leave an open app stale, concurrent read-modify-write actions can lose changes, and an extension can outlive the generation the app considers current.

**Resolve:** distinguish shared durable data from session state, and an app release from each invocation’s pinned generation. Specify app-owned update publication, read-only extension access to immutable generations, retention and failure-accounting ownership, and how external writes become visible. Freeze store-derived render inputs with the timeline entry where consistency requires it. This need not become a general reactive shared store.

**HIGH — D7/D9’s generated Swift declarations are a binary contract.**

Generating `Codable` and TypeScript types coordinates one build; it does not coordinate installed binaries, existing activities, cached widget actions and independently deployed servers. A shape digest cannot rescue a payload that the installed Swift decoder cannot decode. Similarly, an archived generic intent may name an action from an older plan.

**Consequence:** a seemingly updateable Contract change can make activities stop updating or dispatch an old action against incompatible code. Deferring all of this to a compatibility “spec question” leaves a foundational choice unresolved.

**Resolve:** decide now whether activity schemas are fixed per installed binary or carried through a deliberately stable, dynamically decoded envelope. For the simpler fixed-schema choice, include generated activity types, intent declarations and surface capabilities in [LLP 1030 D3a’s compatibility inputs](/Users/ccheever/projects/exact2-wt-surfaces/llp/1030-delivery-unified.rfc.md:247). Servers must target the recipient activity’s supported schema; stale actions need defined acceptance or refusal. D2’s single bake remains valuable, but cannot promise simultaneous deployment.

**MEDIUM — D3/D4/D8 should be conditional on a genuinely headless, extension-safe composition.**

[Q1–Q5](/Users/ccheever/projects/exact2-wt-surfaces/llp/1126-system-surfaces.rfc.md:450) identify the right risks. However, today’s [`ExactSession` initializer](/Users/ccheever/projects/exact2-wt-surfaces/host/apple/Sources/ExactKit/Session.swift:473) constructs a presenter and other UI machinery. “Headless session” requires extraction. Also, [LLP 1113 D3](/Users/ccheever/projects/exact2-wt-surfaces/llp/1113-settled-and-the-two-clocks.rfc.md:108) activates modules and pumps work; its settlement procedure is not D3’s pure rendering contract.

Text is particularly consequential: the current presenter deliberately shares measured line breaks with painting ([`TextAnswers.swift`](/Users/ccheever/projects/exact2-wt-surfaces/host/apple/Sources/ExactKit/TextAnswers.swift:1)). SwiftUI `Text` introduces another layout participant.

**Consequence:** archive restrictions, text disagreement or peak memory could invalidate the proposed implementation even if simple boxes render correctly.

**Resolve:** make stage 0 a decision gate with an explicit fallback. Require a presenter-free composition and preserved semantic leaves. Treat a shared dynamic framework as a measured option: it does not share session heaps and must not defeat [LLP 1047.001’s per-composition linking](/Users/ccheever/projects/exact2-wt-surfaces/llp/1047.001-apple-links-by-use.rfc.md:177). Rust itself is not an evident blocker; the cited approximately 30 MB is not a universal guaranteed allowance.

## Missing surfaces or misclassifications

These affect architectural promises in [§3 and D11–D13](/Users/ccheever/projects/exact2-wt-surfaces/llp/1126-system-surfaces.rfc.md:100):

| Item | Directional correction |
|---|---|
| **Android custom notifications** | These use restricted `RemoteViews`, rather than hosting an arbitrary live Exact view. They belong with remotely rendered descriptions. |
| **Glance widgets and Wear Tiles** | Both have bounded execution, but target different presentation languages: `RemoteViews` and ProtoLayout. One execution class does not imply one rendering backend. |
| **CarPlay templates** | Template presentation can have a live application lifecycle and callbacks. D11 should not universally inherit D7’s intent-or-URL restriction. |
| **Watch complications** | Native watch widgets require watchOS compilation and packaging before the live driver exists. Mirrored iPhone Live Activities are a different deployment case. |
| **App Clips and spatial visionOS content** | App Clips are application targets, not hosted extension controllers. RealityKit immersive content is not covered merely by adding retained state to a two-dimensional SwiftUI renderer. |

Thus, keep **execution lifetime, presentation mechanism and packaging** distinguishable, even if the five labels remain convenient shorthand.

Interactive snippets’ exact archival restrictions remain uncertain to me; their App Intents lifecycle should not be assumed to be WidgetKit’s. I also cannot confirm the HomeView claims from this checkout. Neither should carry the justification for the architecture.

## Alternatives considered

The strongest alternative is **shared Contract data and actions, with native SwiftUI surface views authored or generated separately**. It offers the most direct platform fit and can avoid a general runner inside the extension. It loses unified CSS layout and risks maintaining two presentations of the same component.

I would retain the RFC’s direction unless stage 0 shows that semantic SwiftUI leaves cannot coexist reliably with kernel layout or the required runtime footprint. Translating arbitrary CSS into stacks is weaker for Exact’s stated contract; flattening whole widgets into images sacrifices too much platform behavior.

## Recommended changes to the RFC

1. **Add the ownership and binary-contract decisions above.** Preserve D2, D3, D6 and D7, while making “one app” a source/release relationship rather than a claim of one simultaneously running state.
2. **Strengthen stage 0’s acceptance criteria.** Exercise an actual archived widget and a minimal Live Activity region, including changing time text, accessibility, platform rendering modes and an intent. Measure peak memory and cold execution with TypeScript refresh work as well as Caltrain’s Rust path. Include concurrent app/extension activity. A gallery alone cannot establish these properties.
3. **Correct the watch sequencing.** Widgets → Live Activities → other supported WidgetKit targets → live watch app remains reasonable, but watch compilation and scope approval must precede native complications. [DEFERRED §Surfaces](/Users/ccheever/projects/exact2-wt-surfaces/rules/DEFERRED.md:354) excludes watchOS itself, and [1035.006 item 11.10](/Users/ccheever/projects/exact2-wt-surfaces/llp/1035.006-public-ui-coverage.plan.md:324) explicitly avoids admitting platforms by implication.
4. **State the binding boundaries explicitly.** D5 inherits LLP 1115’s accessibility precedence as well as author/platform precedence. Refresh execution needs a named lifecycle boundary; it cannot reuse “first pixel” mechanically. Keep the new renderer optional and use existing data/module seams, including [1124’s proposed integration](/Users/ccheever/projects/exact2-wt-surfaces/llp/1124-host-extensions.rfc.md:170), without creating another capability system.

## Open questions for the author

- Must a widget action update an already-open app immediately, or is visibility at a defined refresh boundary acceptable?
- Must Live Activity schemas change through delivered plans, or can schema changes require a new binary?
- If semantic text plus CSS placement fails the archive probes, should Exact narrow the admitted surface vocabulary or permit separately authored native SwiftUI views?
- Is watchOS admitted for complications before the full watch app, or should both wait for the same ruling?
