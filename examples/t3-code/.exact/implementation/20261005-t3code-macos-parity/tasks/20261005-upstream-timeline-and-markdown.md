---
name: 20261005-upstream-timeline-and-markdown
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-upstream-timeline-and-markdown
pr_url: null
verified_commit: null
---

# The conversation timeline matches T3 Code 1e2ecbd975

## Outcome

A person reading a thread sees what the reference desktop app shows at `1e2ecbd975`:

- A send past "Resume with less context" sends only the typed message. No automatic `/compact` turn runs first.
- An expanded command or tool row shows the call in normal text above a muted result. There is no card and no "Input" or "Output" heading. The result loads on demand, with the reference's loading, empty and error states. A row with nothing to show does not expand. A non-zero exit shows a red `exit N`.
- A file link with a descriptive label keeps its words and gets the file chip after them.
- A tool row from an integration (MCP) carries the tool's own icon.
- `$skill` words in messages are skill chips.
- A real scroll of the timeline closes a hovered tooltip.

Code colours for more languages, long texts and italics are in `20261005-shiki-residuals`.

## Scope and exclusions

Clone paths are `examples/t3-code/<file>`; the tables write `<file>`. Line numbers come from the mc-orch tree, 2026-10-05. Reference paths are repo-relative at T3 Code `1e2ecbd975` (`W/` is `apps/web/src/components/`). IDs are from [research](../research.md).

| ID | Reference behavior and states | Reference evidence | Clone state and work |
| --- | --- | --- | --- |
| A1 (`2188bdd8b5`) | Remove the automatic compaction. The banner ("Resume with less context", token count, "Compact" button) stays. | `W/ChatView.tsx:8877` (`shouldQueueBehindActiveRun`), banner `:7290`, `onCompactContext` `:8196`. No tests. | **Present, remove.** `client.ts:20,895-902` (`compactBeforeSend`, `compactTurn`, forced `queue` mode), `r3-composer-controls-resume.ts:67-81`, test `r3-composer-controls-resume.test.ts:63-70`. Keep `cc:compact` (`composer-controls-commands.ts:175-185`) and the banner (`composer-controls-view.ts:188-193`). Delete code that no caller uses afterwards. |
| A2 + CN1 (`5a96895a85`) | An expanded command or dynamic-tool row fetches its output. States: "Loading output…" (italic, muted); "Couldn't load output: {error}" (destructive); "Output is no longer available." (item gone); "No output." (fetched, empty; italic, muted). Read and skill rows keep their plain text and still fetch output the newer server withheld (CN1). A row with no call, no output and no non-zero exit has no disclosure. A failed file edit shows the provider's error text (`diffStr`). A search with only a pattern shows the pattern. | RPC `orchestration.getTurnItem` (`packages/contracts/src/orchestrationV2.ts:2980`, input `:3267`, result `:3276`; `outputOmitted` `:1360,1488,2096,2221`). Server strips command and tool output from the wire whenever it is non-empty: `apps/server/src/orchestration-v2/WireProjection.ts:89,115`. `packages/client-runtime/src/work-log/itemDetail.ts:136,165,207` (`turnItemNeedsDetailFetch`, `turnItemOutputText`, `turnItemHasDetail`, `turnItemDetailRevision`). Cache: `client-runtime/src/state/orchestration.ts` `turnItem` family, 60 s stale and idle. `W/chat/V2ItemInspector.tsx:99-148`, `W/chat/MessagesTimeline.tsx:5124-5176,5340-5372`. | **Missing.** Today `timeline-worklog.ts:489-504` shows the input and an exit line only, and `timeline-presentation.ts:231-235` lets any titled row expand. Reuse `domain.ts:143` (`output`), `timeline-inspect.ts:27-32` (`plainOutput`), and the on-demand fetch shape of `diff.ts:86` (`orchestration.getTurnDiff`). Port `itemDetail.ts` as `timeline-item-detail.ts` with the reference names. |
| A3 (`df4ae529ea`) | Expanded body: the call (full command, or `key value` lines, or formatted JSON) in foreground text above the muted result (max height 320). No card, no headings. `exit N` in red only when N is not 0; exit 0 alone does not make a row expandable. | `itemDetail.ts:82-117` (`toolCallLines`, declared at :82), `V2ItemInspector.tsx:140-190`, `W/chat/WorkLog.tsx:126`. No tests. | **Missing.** Card, border and "INPUT" heading: `timeline-work.contract:297-318`, `timeline-inspect.ts:44` (`inspectorCode`). Replace "Process exited with code N" (`timeline-worklog.ts:492`). |
| A4 (`1d013c7ca1`) | A file link whose label is a filename, a path or `name:line` shows only the chip. A prose label stays, followed by the chip. Copy keeps the source `[label](href)`. | `packages/client-runtime/src/markdownLinks.ts:268-288` (`isMarkdownFileLinkLabel`, declared at :269), `W/ChatMarkdown.tsx:3238-3254`. Test `markdownLinks.test.ts:13` (16 cases). | **Missing.** `macos/src/markdown.rs:82-108` (app Rust crate) builds the chip from the href only; `r4-timeline-chips.ts:57-72` lists chips; `markdown.contract:397,430`. |
| toolSource / toolIcon (`ec20db4a5a`, gap predates the range) | A tool row shows `toolIcon ?? toolSource.icon`: `website` (favicon from the page origin, no third-party service), `themed-logo` (dark variant in dark), `native-app` (`assets.createUrl` resource `native-app-icon`). The fallback glyph shows while an image loads or fails. A failed row with an image icon shows a trailing x instead of a tint. Warning and severe rows ignore the icon. | `W/chat/MessagesTimeline.tsx:3394,3510-3600,3650,4587-4700,5090-5100`. Schemas `packages/contracts/src/providerRuntime.ts:399-436`, `orchestrationV2.ts:1257-1258`, `assets.ts:50`. `packages/shared/src/favicon.ts:61` (`toolActivityFaviconUrl`), test `favicon.test.ts:44-100`. | **Missing.** No `toolIcon`/`toolSource` in the clone. Reuse the `assets.createUrl` pattern (`r3-sidebar-glyph.ts:53`, `r5-panels-attach.ts:90`). Port `toolActivityFaviconUrl` with its tests. |
| G12b | `$name` in message text becomes a skill chip when `name` is in the provider's skills (glyph + display name; copy gives `$name`). Not inside code spans or links. A number or price (`$5`, `$1k`) is not a chip. | `W/chat/SkillInlineText.tsx:9-60` (`SKILL_TOKEN_REGEX`), used at `MessagesTimeline.tsx:2255,2515,4367-4455,4954` and `ChatMarkdown.tsx:2938,2975`. `client-runtime/src/providerSkills.ts:22` (`formatProviderSkillDisplayName`), test `providerSkills.test.ts:37`. | **Partial.** Chips from `t3-context://v1/skill/…` links exist (`r4-timeline-chips.ts:94`, `markdown.contract:374,488`). Plain `$name` tokens are not matched. Skill list: `composer-editor-menu.ts`, `composer-editor.ts` (`snapshotFor`). |
| A17 (`32b77f4fa3`) | A real scroll event in the timeline closes the last hovered tooltip. A tooltip whose trigger holds the keyboard focus stays. A wheel without a scroll does nothing. | `W/ui/tooltip.tsx:16-80` (`TooltipScrollDismissArea`), `MessagesTimeline.tsx:1328`. Tests `MessagesTimeline.test.tsx` "timeline tooltip scroll dismissal" (hover then focus, wheel without scroll). | **Unverified.** Tips are `Tip` components with local hover state (`shell-tip.contract:7`, `r8-pointer-tips.contract`). The PR preview card half of the reference change belongs to `20261005-pr-links-previews-and-routing`. |

Not in this ticket (no work): `0ce43a4eea` and its revert `801ca76ed6` (net zero; the clone has no link repair, keep it that way); `d3bec62ec1` (macOS has a hovering primary pointer); `2dfef8779e` (needs the excluded Browser surface); Shiki residuals (languages, long texts, italics, the generator: `20261005-shiki-residuals`); rendered HTML from non-IP `http` hosts (X7, `20261005-media-actions`); code-line wrap points (X10).

States for the new UI: loading, empty and error are in the A2 row. Disabled: a row without detail has no disclosure, its chevron is hidden and a press does nothing. Hover: timestamp reveal (exists). Keyboard focus: the row button is focusable and already exposes `aria-expanded` and an `aria-label` (`timeline-work.contract:274`); keep both. Space and Return toggle the disclosure. Escape: a tooltip needs no key to close and the disclosure has none; focus stays on the row. Permission: none (read scope). `aria-label` on the tool icon image: the row's label. Motion: the chevron turn (exists); no new motion, so reduced motion is unchanged. There is no dialog, menu or popover in this ticket; a tooltip is a hover card without actions.

## Context and guidance

Parent specification: [spec](../spec.md). Research: [research](../research.md). Source behavior: the reference files above; the committed `EXACT2-GAPS.md`.

Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `ref-build.sh`, `electron-oracle.mjs`, `trace-proxy.mjs` and `trace-diff.mjs` come from `20261005-desktop-oracle-and-trace`; the agent driver is `bun scripts/agent.mjs macos --json …`.

Library revision: `20261005-platforms-v3`. Selected topics:

- state-and-data: a per-row fetch is a keyed read; a late reply for an old key must not overwrite a new one; keep the open row's content visible during refresh.
- layout-and-interaction: variable-height rows, bounded scroll for the result, long text.
- design: complete states.
- accessibility: disclosure semantics.
- testing-and-debugging: one session per observation; `--json`.

Unknown in the library: the app's own hooks (`t3-transcript`, `t3-rehover`, `t3-turn`) and native views; the Rust Markdown crate `macos/src/markdown.rs` is app code. The clone's runtime evidence on the pinned main (`20261005-clone-on-exact2-main`) is the basis for those.

Consumer framework revision and toolchain: the pin chosen by `20261005-clone-on-exact2-main`; pinned Bun 1.4.2 and Hermes. The fixture runtime must be the reference runtime at `1e2ecbd975` or later (from `20261005-desktop-oracle-and-trace` `ref-build`): the current `runtime-f870c41` predates `orchestration.getTurnItem` (the commit `5a96895a85` is newer than `f870c419fc`).

Logic reuse (user rule): port `itemDetail.ts`, `markdownLinks.ts`, `SkillInlineText`'s regex, `formatProviderSkillDisplayName`, `toolActivityFaviconUrl` with their function names and original test names (`bun:test`). `itemDetail.ts` has no reference test: write tests named after its functions and mark them clone-authored in the header. Record each change for exact2 in the file header. Keep one copy of the A4 rule: either the Rust parser runs the ported rule, or TS decides and passes it through the chip list. Decide at `prepare`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle shots, trace diff, runtime at the new pin) | pending |

Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (A17 touches tooltips). `20261005-hot-file-split` is a merge prerequisite (see the table). It makes room in `client.ts` (1,455 of 1,500 lines) and `app.contract` (1,327) for the new fetch. The A1 removal also frees lines in `client.ts`.

## Issue assessment at preparation

Checked sources and time: local issue drafts in [issues](../issues/README.md), `EXACT2-GAPS.md`, the library; no upstream search (planning). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Resource in a child component, or a root across files | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines; this ticket adds a fetch resource and state | nonblocking until the cap (workaround: `client-ops-*.ts` and child components, which `20261005-hot-file-split` prepares) | Remove A1 code first; measure lines before adding |
| [X13](../issues/20261005-x13-hover-keys-during-pan.md) | Hover events during a pan | A17: a scroll while the pointer rests | nonblocking | Run the A17 drive; record the result |
| [X24](../issues/20261005-x24-still-pointer-rehover.md) | Hover re-test under a still pointer | The `t3-rehover` workaround may reopen a tip right after the scroll | nonblocking | Assert the tip stays closed until the pointer moves |
| [X10](../issues/20261005-x10-text-rendering-parity.md) | Text rendering parity | Row text wrap points and weight differ in pixel pairs | nonblocking | List in `EXACT2-GAPS.md` with the pair |
| none found | A1–A4, tool icons, skill chips | Contract already draws images and `font-style` (`markdown.contract:324`); check the image element with `contract vocab` at `prepare` | none | — |

## Implementation notes

- A2 data flow. An expanded row asks for `(environment, thread, item, revision)`; revision is `live` while the item runs and `updatedAt` after. Cache 60 s. Show content already loaded while a newer revision loads. Only fetch when `turnItemNeedsDetailFetch`.
- A2 view fields: add an output state (`loading`, `error`, `empty`, text) to `Activity`. Keep `timeline-work.contract` under 1,500 lines.
- A3: delete the card column, the "INPUT" heading and the green or red exit text. Result text is `light-dark(#71717b, #818181)`; the call is the foreground colour at 85%.
- Tool icons: cache loaded sources across rows (`loadedToolActivityIconSrcs`); re-mint native-app URLs as `r5-panels-attach.ts` does. Image drawing uses the Contract `image` element; test that a broken URL falls back.
- A17: a child component cannot assign root state, so pass a dismiss serial down as a prop. Tips clear their hover when it changes, and ignore it while their trigger has focus.

## Acceptance and reproduction

All rows: macOS 26.6.2, 1280×840 and 840×620, light and dark. Pixel pairs use the reference desktop oracle. Isolated fixture backends use ports 16000–16999 and isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_* and T3CODE_HOME.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| A1 no automatic compact | Synthetic projection that shows the banner (the fixture provider cannot produce context usage) | Send past the banner | One send, no `/compact` message; banner and Compact button unchanged | macOS | rewritten unit test, trace diff, tree |
| A2/A3/CN1 tool rows | Fixture thread with a command that prints output, a dynamic tool, a read and a skill; runtime at the new pin | Expand each row; expand a row with no content; expand a failed command | Loading → output; no card or headings; `exit N` only when not 0; empty row has no chevron; a forced RPC failure shows "Couldn't load output: …" | macOS, pointer + keyboard | agent transcript, shots beside oracle, trace shows one `orchestration.getTurnItem` per open |
| A2 late reply | Two rows opened quickly | Open A, then B before A answers | Each row shows its own output | macOS | transcript |
| A4 file links | Message with `[parser](src/a.ts:3)`, `[a.ts](src/a.ts)`, `[src/a.ts:3](src/a.ts:3)` | Read the row; copy | Prose + chip; chip only; chip only; copy keeps the source | macOS | shots, ported tests (16) |
| Tool icons | Synthetic items (the fixture cannot produce MCP icons) with each icon kind and a bad URL | Render | Image; fallback while loading and on error; trailing x when failed | macOS | unit tests, shots; live check with an MCP provider: unverified until a provider is available |
| G12b skill chips | Thread with `$name`, `$5`, code span `` `$name` `` | Read | Chip only for a known skill outside code and links | macOS | ported tests, shots |
| A17 tooltips | Long thread | Hover a timestamp, scroll with the agent; repeat with keyboard focus | Tip closes on scroll; stays when focused; stays closed under a still pointer | macOS | transcript |
| A17 real wheel `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773 by default, see `20261005-embedded-server-runtime`); a long thread | Hover a timestamp, scroll with a real wheel | Same result as the agent row | macOS | `(attended session)` notes, shots |
| Disclosure keyboard | Fixture thread with tool rows | Tab to a row, press Space, then Return | The row opens and closes; focus stays on the row; the result region is reachable by Tab | macOS | `tree --ax`, transcript |
| Checks | `git add -A` | clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit binaries), `bun scripts/caps.mjs`, the five checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `client.ts`, `r3-composer-controls-resume.*`, `timeline-*.ts`, `timeline-work.contract`, `markdown.contract`, `macos/src/markdown.rs`, `r4-timeline-chips.*`, `shell-tip.contract`, new `timeline-item-detail.*`, `markdown-links.*`, tests, `AGENT-HANDOFF.md`.
Required environment: the reference runtime at the new pin; no Chrome-only tooling; no real provider needed except the optional live icon check.

## Progress

Implemented on the round-12 snapshot `1c6b4a12a` under the user's 2026-10-06 direction to work on independent tasks concurrently. This branch retains framework `c1522fdac` plus the existing app parking patch; the main migration, hot-file split PR and new desktop oracle are not represented as merged. The coordinator integrates shared source/manifest changes and runs the combined native build. No pixel-perfect comparison loop was run.

- A1: removed the automatic-compaction helper; the coordinator removes its call/import in `client.ts`. Explicit Compact remains in `composer-controls-commands.ts`. The send regression now expects only the typed message.
- A2/A3/CN1: ported `itemDetail.ts` with original function names; full call text above bounded muted output, no card/headings, only nonzero `exit N`. Read paths and skill arguments remain visible above fetched output. Search patterns and failed edit errors are preserved. Empty rows have no disclosure.
- The keyed detail cache isolates connection generation/origin, environment, source thread, item and live/final revision, deduplicates requests, expires after 60 seconds, retains successful content during refresh, and rejects mismatched replies. Disclosure state is data-owned; an independent root mutation awaits reads while the snapshot remains usable. All cache clocks are supplied by the host through `composerNow`, never `Date.now()`.
- A4: the sole `isMarkdownFileLinkLabel` rule is in app Rust, with the reference's 16-case table. Prose labels remain before file chips; source copy is unchanged.
- G12b: provider-known `$skill` tokens become chips outside code and links in paragraphs, lists, headings, quotes and tables. Table copying restores the original token. The coordinator wires the existing cwd-scoped provider skill snapshot into `renderMarkdown`'s second argument.
- Tool icons: website/themed/native-app source precedence and asset URL refresh are implemented. An app-owned native overlay loads/caches image data and uses documented `ExactElement.click()` to change Contract fallback state; it never mutates Exact-owned subviews. Warning/severe rows retain their glyph and failed image rows add an x.
- A17: timeline-only tooltip hooks observe actual clip-origin changes, preserve focused triggers and remain dismissed under synthetic rehover until physical pointer motion. A wheel that does not scroll does nothing. Contract owns tooltip state; native only emits its declared press callback.

Guidance remains library revision `20261005-platforms-v3`: foundations, state/data, layout, design, accessibility and testing, using application-owned native hooks where the pinned Contract vocabulary has no image-load/scroll-tooltip callback. No framework edit.

Shared integration: `timelineReadsNeeded(client)` and awaited `refreshTimelineReads(client, native)` from `timeline-prepare.ts`; snapshot fields `timelineReadsNeeded` and `markdownSkills`; source `refreshTimelineReads`; `noteNow` before reads; hooks `t3-tool-icon` / `t3-timeline-tip`; data keys `tool-icon-light` / `tool-icon-dark`; install/remove/destroy the two new app Swift hook owners. The Rust mixed-source allowlist includes the timeline mutation and parallel activity source names.

Task stays active/unverified. The fixture runtime `f870c41` lacks `orchestration.getTurnItem`; deterministic protocol tests prove the implemented request path, not runtime parity against a newer server. Native interaction, provider-backed MCP icons, physical-wheel/focused-tooltip checks and output-region Tab reachability remain to verify. The pinned Contract compiler does not expose `tabindex`/`tabIndex`, so no unsupported attribute is emitted.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 2026-10-06 independent implementation | round-12 `1c6b4a12a`, task branch above; Bun 1.4.2 | 1,170 Bun tests pass, strict README `tsc` passes, Contract build returns `[]`; Rust formatting and diff checks pass. Checks temporarily applied the coordinator's A1/manifest seam changes, then restored those shared files. AppKit harness: 5 image lifecycle and 4 tooltip origin/latch checks pass with actual AppKit views and a minimal ExactElement stub. | Local-only logs `/tmp/t3-timeline-checked-0.log`, `/tmp/t3-timeline-checked-1.log`, `/tmp/t3-timeline-final-0.log`; harness `/tmp/t3-tool-icon-check`. Feature regressions committed alongside code. | Full integrated native tests/build and newer fixture runtime pending; no visual or live-input acceptance claim. |

## Next action

Verification remains `blocked` after the authorized repair pass. The final native frame-ID replay passes the output boundary regression; read overlap and the remaining attended tooltip acceptance lack valid live proof. See the final repair evidence below. Keep this task active; no closure or PR publication.

## Combined integration, 2026-10-06

Integrated in `daehyeon/t3code-parallel-features` with shared root/native registrations.
Combined validation: 1,197 Bun tests, strict TypeScript, Contract compilation, 10 app
Rust tests, formatting, staged caps and boot passed. The tab Contract hooks use the
pinned compiler's supported vocabulary; timeline activity fields satisfy generated
Contract types. Independent code reviews completed. No pixel-fidelity loop was run.
Main migration, newer oracle runtime and live feature acceptance remain pending;
this evidence does not close the task's verification gate.

Integrated native bundle build passed (app Rust bake and full Swift module). The
isolated macOS driver launched the app and read its disconnected tree. The bounded
interaction check did not pass: the first requested welcome target was absent; the
actual Open Connections target was outside the default viewport, then reported hidden
or inert at 1280×900. Stopped after three attempts without UI adjustment. App interaction
acceptance remains unverified. Local evidence: `/tmp/t3-parallel-final-native.log`,
`/tmp/t3-parallel-final-native-tests.log`, `/tmp/t3-parallel-final-bun.log`, and
`/tmp/t3-parallel-final-smoke.log`. No push or PR publication performed.

## Exact skill verification, 2026-10-06

**Result: failed.** 158 Bun,10 Rust and11 native hook assertions pass; real pinned server output fetched and shown. Actual ExactKit Tab loop skips tool-output scroll region (acceptsFirstResponder/canBecomeKeyView/tabbable false). This is a functional acceptance failure. Remaining detailed live rows are listed in evidence.

See [live attempt](../evidence/parallel/20261006-live-verification/attempt.md), [capture report](../evidence/parallel/20261006-live-verification/checks-final/report.json), and [independent review](../reviews/20261006-parallel-verification.md). Source remained unchanged. No framework issue was resolved or closed by this app-only verification.

## Authorized repair pass, 2026-10-06

User explicitly requested repair and continuation through verification. Prior failed evidence remains preserved. Work uses Exact implement/verify guidance and stays app-owned; no framework changes or pixel-polish loop.

## Final targeted repair evidence, 2026-10-06

Verification **blocked**, not closed. The final named native bundle passes the loaded-output boundary:334.88→334.88 after three Down keys→294.88 on the first Up. Actual native frames and source receipt: [frame-ID replay](../evidence/20261005-upstream-timeline-and-markdown/20261006-repair-verification/frame-id-agent/observations.md). Actual Tab/disclosure reachability and known-skill/rendering evidence remain in the preceding repair packets.

The independent read slots preserve per-row output ownership. The [overlap attempt](../evidence/20261005-upstream-timeline-and-markdown/20261006-repair-verification/two-slot-agent/observations.md) cannot prove concurrent admission: the deterministic agent clock awaits A's replies before it advances the `then` callback for B. Independent review identifies this as a harness limitation, not a production serialization finding. Normal live mode is blocked on credential keychain insertion; no user keychain was modified. Focused-tooltip and attended wheel acceptance remain incomplete. Prior failed evidence is preserved; no pixel-perfect loop was performed.

## Wave 3 repair: Tab order, 2026-10-06

Base `9670b0723` (feature branch tip). The 2026-10-06 failure ("Tab skips the tool output")
was reproduced on the actual host and in the mounted app, then fixed in app code.

**Root cause.** Each work row held three extra Tab stops ahead of its output: the
`t3-tool-icon` hook (16×16, `aria-hidden`), the row timestamp (it had `focus`/`blur`
handlers) and the `t3-timeline-tip` hook inside it (zero size, `aria-hidden`). ExactKit
makes a node with a `press` handler tabbable, so both hooks were invisible stops; one Tab
from an open row landed on nothing visible. The reference timestamp is a plain span
(Base UI 1.5.0's `TooltipTrigger` adds no `tabIndex`), so it is not a stop there either.

**Fix.** `tabindex=-1` on the four hook boxes (`shell-tip.contract` ×2,
`r8-pointer-tips.contract`, `timeline-icons.contract`); `TimelineTimestamp` is hover-only
and is revealed by row hover, focus or expansion; `ToolOutput` is a focusable scroll
region only for loaded output (`max-h-80 overflow-auto`), and loading, empty and error
are plain text, as in `V2ItemInspector.tsx` `ToolOutput`. A gone item now reads
"Couldn't load output: Output is no longer available." in red, the reference's text.

**Checks.** `bun test examples/t3-code` 1,829 pass / 1 skip / 0 fail (two new source
tests in `timeline.test.ts`); strict tsc clean; `contract build` 2,320 slots, 43
resources; `cargo test -p t3-code-macos --lib` 10/10; `timeline-keyboard` (actual
ExactKit host) passes: it now routes keys through `Presenter.routeKey` as the session's
monitor does (the old direct `keyDown` call no longer reached `key` handlers on this
framework revision) and reports "Base row: 3 invisible or non-reference Tab stop(s)
before the output" for the old structure; five checks pass (2,926 Rust tests, clippy,
fmt, caps, boot); macOS bundle builds.

**Live drives** (one BEFORE on `t3-code-evidence-base`, one AFTER here; synthetic fixture
thread `verify-timeline` on the reference server `1e2ecbd975`, port 16301, isolated homes;
fixture and drive script under `target/t3-fixture/`, not committed). The before drive
ran three times: the first two stopped in the driver script (empty import step; the
signed-out Codex warning covering the group header), not in the app.

```
BEFORE  Tab walk from work-detail-[…fixture-command]:
  [983 View hidden] → [985 work-timestamp-…] → [987 View hidden] → [1179 work-output-… "Tool output"]
AFTER   Tab walk from work-detail-[…fixture-command]:
  [1180 work-output-…command "Tool output"] → [995 work-detail-…exit] → [1186 work-output-…exit] → [1013 work-detail-…read]
AFTER   a fifth Tab → [1191 work-output-…read]; Shift-Tab there → [1013 work-detail-…read]
AFTER   Space on command row → expanded false, focus stays on 977; Return → expanded true, focus stays on 977
AFTER   outputs (getTurnItem): command "verified output\nsecond result line"; exit "verification failure result" + red "exit 2";
        read "Timeline verification fixture"; skill "Skill fixture output"; dynamic 31 lines (bounded scroll);
        empty row: expanded false, no output node (no disclosure)
```

| Row | Result |
| --- | --- |
| Disclosure keyboard | **Pass** (live): one Tab reaches the output with its ring; Space/Return toggle; focus stays on the row. |
| A2/A3/CN1 tool rows | **Pass** (live) for command, failed command, read, skill, dynamic and empty rows. Forced RPC error and missing item: unit tests only. |
| A2 late reply | Unit tests only (not driven). |
| A4 file links | **Pass** (live): "Review parser [fixture.txt · L3], [fixture.txt], [fixture.txt · L3]". |
| Tool icons | **Pass** (live, synthetic items): website, themed, native, broken → fallback glyph, failed → trailing x. Provider MCP icons: unverified (needs a provider). |
| G12b skill chips | Rust tests pass; live `$verify` stays plain in both builds because the fixture's Codex is signed out, so no skill list: unverified (needs sign-in, on hold). Price, code span and link exclusions show plain. |
| A17 tooltips | Unverified: the agent hover did not open the timestamp tip on either build (tip stayed `aria-hidden`), so the agent scroll-dismiss row has no observation. Real wheel: unverified (attended). The timestamp is no longer a focus trigger (reference parity); the focused-trigger case applies to button triggers and was not driven. |
| A1 | Unit test only (not re-driven). |
| Oracle / trace diff | Not run (oracle not built). |
