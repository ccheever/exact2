# LLP 1109: T3 Code on iPhone and iPad

**Type:** RFC
**Status:** Draft
**Systems:** Contract, TypeScript data sources, Apple host, T3 Code mobile app
**Author:** Nate's implementation session
**Date:** 2026-10-07
**Implementer:** This implementation session, starting 2026-10-07
**Related:** LLP 1008 §9, LLP 1027, LLP 1038, LLP 1109.000

## Goal

[confirmed] The supplied handoff, 2026-10-07, authorizes an iPhone and iPad port
of T3 Code mobile at `365aa87982`, alongside Daehyeon's desktop client. It also
authorizes these design documents. The app must match the upstream routes and
showcase scenes in both appearances, with measured performance against the same
upstream release build. Android and changes to Exact itself are outside this task.

[confirmed] On 2026-10-10 the user resumed the clone goal and added final
TestFlight delivery. After functional and visual acceptance, build the native
iOS app and use EAS Submit to upload that identified archive. Verify Apple
processing and TestFlight availability before claiming delivery. This authorizes
the beta upload, not a public App Store release. The release remains pending;
resuming the overall goal does not reopen separately stopped correction targets.

[confirmed] The continued direction keeps app changes on draft PR216, with
reviewed commit and push checkpoints. A suspected Exact framework defect gets
its own subagent and worktree from current remote main. It must have a concrete
reproduction, a rules review and evidence for the fix before a draft PR. A larger
framework decision belongs in a GitHub issue for Charlie. Example-app gaps do
not become Exact framework issues. The separately authorized selection/Refresh
correction remains one bounded candidate and check.

## Architecture

[observed] `js/bake/src/lib.rs` reads `app.contract` from the app directory.
The mobile root therefore lives at `examples/t3-code/mobile/app.contract`, with
`app.json`, `app.ts` and an `ios` crate. The layout decision is
[LLP 1109.000](1109.000-mobile-app-layout.decision.md).

[confirmed] The handoff requires reuse of the desktop client. Mobile was intended to import
parent TypeScript modules directly. It currently imports an unchanged pinned
copy, because the bake refuses that layout. Shared behavior is not forked. An
unavoidable shared export must be additive and land in a separate `shared:` commit.
The parent import failed in the bake. Gap 001 requires a pinned copy of the
shared parser under `mobile/shared/`, with its source SHA and license retained.
It is a bake probe, not the finished connection screen.

[inferred] The iOS transport adapter should reuse the desktop Foundation code
where it compiles, with UIKit implementations of platform services in the mobile
directory. The transport cannot yet be described as portable: its activity
reporter uses AppKit, and remote authentication identifies the desktop platform.
M2 must verify every reused dependency and the mobile authentication identity.

[observed] Upstream `packages/contracts/src/environment.ts` at `365aa87982`
declares orchestration protocol 2, the version the desktop client implements.
This confirms the protocol number only. It does not prove every RPC shape agrees
with the desktop reference at `f870c419fc`.

## Parity method

[confirmed] Every route in upstream `apps/mobile/src/Stack.tsx` is in scope,
including sheets. Captures use the same simulator model, appearance, text size,
locale, status bar and seeded data. Each pair is inspected with a pixel diff and
an interaction drive. A built route is not marked matching until that evidence
exists.

[observed] The pinned showcase starts three seeded environments. The mobile
oracle must follow `scripts/mobile-showcase.ts`, including its capture build
identity, rather than assuming a generic development client reproduces it.

[confirmed] Framework gaps stay in local handoff files. Existing desktop gaps
receive iOS findings under their existing issue and X-id. App workarounds cite
those entries. No framework files or working-set links are added by this task.

## Performance

[confirmed] Compare release builds on the same simulator and data: cold launch
to meaningful content, long-thread opening and scrolling, composer keystroke
latency, and memory after ten threads. The target is faster than upstream on
each measure. No performance result has been measured at this revision.

## Current acceptance boundary, 2026-10-11

[observed] Checkpoint `aba91d670` contains the current mobile implementation.
Its full mobile suite passes 2,941 tests; the repository suite passes 3,521 with
34 ignored. The native app builds and the installed Mini bundle matches all
89 files. These results establish their checked code and runtime boundaries,
not completion of the route, interaction, appearance or performance matrix.
Later implementation and native evidence live in the numbered decisions;
the initial milestone notes below are historical, not the current inventory.

[observed] The range checkpoint retains the long-press anchor through repeated
endpoint taps and passes a read-only Mini check against the actual laptop.
The latest Usage checkpoint verifies conditional refresh-control composition,
empty-state navigation and a populated actual-laptop visit. Exact refresh
dispatch and spinner timing remain unverified. The saved-history experiment
reconstructs the full saved response, retains both graphs correctly and
reclaims the graph-sized remainder after the JS function returns. Production
ingestion and device capacity remain unverified. These boundaries are in
[Settings](1109.009-mobile-settings.decision.md) and
[Composer and transcript](1109.005-composer-and-transcript.decision.md).

[open] Full native parity and the release performance comparison remain
unfinished. Individually stopped targets retain their recorded limits and
need new direction before another correction. The verified delivery destination
is Expo `@exponathan/t3-code` / ASC app `6821176299`; no final accepted IPA or
EAS submission has been produced. TestFlight delivery follows functional,
visual and performance acceptance rather than replacing it.

[observed] A current-main audit at `5f50e987` confirms that URL schemes and
file-opening declarations work, while the draft `share_target` declaration is
rejected. The generated iOS build has no Share Extension/App Group packaging
path. Existing draft LLP1114.006 already owns that framework decision;
[issue 473](https://github.com/ccheever/exact2/issues/473) adds the tested
manifest reproduction and T3 as another consumer. The local incoming-share
inbox does not establish an OS share-sheet handoff. No framework patch or
availability claim follows from this audit.

[open] The final release's encryption declaration still needs a complete
archive inventory. The native builder already accepts
`EXACT_NON_EXEMPT_ENCRYPTION`, but the source graph includes the Hermes
executor's ring-backed WebCrypto as well as Swift's OS services. Swift HTTPS
and Keychain usage alone cannot establish the complete binary's classification.
No declaration or upload was made. The read-only source inventory is retained
in `t3-eas-destination-20261011/delivery-check/encryption-source-audit.md`.

## Initial implementation evidence

[observed] M0 established a Contract root, a TypeScript source importing the
shared pairing parser, and a `t3-code-ios` crate registered in the root workspace.
Three Bun source tests pass, including a check that the copied parser still
matches the adopted desktop source. The direct parent import fails with TS2307 in the
bake. The pinned-copy native bake and iPhone launch passed. A driver field edit
produced `https://server.example`, with no pending or failed requests, and a
screenshot confirms it is drawn.

[observed] M1 now has the reviewed library map,1872 pinned color tokens, bundled DM Sans files and iOS image assets. The oracle has30 clean ordinary showcase images and66 route capture attempts, distinguishing redirects and guards. Upstream activity stubs and missing fixture records prevent the remaining captures. These are oracle evidence, not Exact parity.

[observed] M2 adds the shared client closure, UIKit transport, connection forms, scanner, environment details and permission-aware maintenance. M3 has a Home projection and shelf persistence; native chrome is still being integrated. The root Contract, app Cargo checks and focused helper tests have passed at preparatory checkpoints. Three full native build failures after a worktree relocation exhausted the repository's fix loop; the prepared module-discovery guards typecheck, but another full native build awaits a human decision. M2 pairing and all later screen parity remain unproved. M4 transcript preparation is not yet connected to the root. M5–M11 remain work.

Code: [root](../examples/t3-code/mobile/app.contract),
[source](../examples/t3-code/mobile/app.ts),
[bake](../examples/t3-code/mobile/ios/build.rs).
Local progress, route inventories and evidence live in `.context/t3-code-ios/`
in the implementation worktree; these are not published artifacts.

## Open questions

- Gap 001: the bake refuses parent imports. When can the pinned copy be removed?
- Will live pairing, credential persistence and lifecycle drives confirm the prepared UIKit adapters?
- Which native controls and navigation forms match the pinned reference on both devices?
- Can every performance target be met within app scope?

An independent Codex collaboration session reviewed M0 and its maintenance
links. The [received review](reviews/1109-t3-code-ios.codex.md) found no high-severity
issue; its wording and copy-maintenance corrections were applied. Grok and Gemini
are unavailable on PATH. The CLI attempt failed before returning a review. The
received review is from the author's family, not a two-family refinement.


## Number allocation after base update

[observed] The 2026-10-07 rebase onto d82fb6a introduced upstream LLP1106, the semantics prompt research. This mobile family was previously allocated1106 on its separate branch. The fetched corpus and reachable history reserve no higher number. The mobile family now uses1107, preserving child identifiers, review provenance and Draft statuses. Upstream LLP1106 and the current/foundation links are unchanged.
