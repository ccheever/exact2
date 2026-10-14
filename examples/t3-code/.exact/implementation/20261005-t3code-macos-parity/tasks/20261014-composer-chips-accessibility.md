---
name: 20261014-composer-chips-accessibility
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: stale
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-composer-chips-accessibility'
pr_url: https://github.com/ccheever/exact2/pull/534
verified_commit: null
---

# The composer's chips to VoiceOver, the terminal chip's glyph, the context popover's order

## Outcome

The user asked on 2026-10-14 to fix what is fixable among `realinput-1011c`'s known differences (STATUS "Known
differences", tables 2 and 3); the coordinator's triage (fix wave, clone task `composer-chips-accessibility`) gave this
task these rows. The user decided on 2026-10-11 that every row is built now, matching the reference.

| Id | Clone (`realinput-1011c`) | Reference (check it first) |
| --- | --- | --- |
| G1-8 | The terminal's Add to chat chip draws a speech bubble (the generic context glyph) and is no accessibility element: the Message area's value is the Markdown link `[Terminal 2 lines 1-738](t3-context://v1/terminal/…)` ([G1-8b-pair.png](https://raw.githubusercontent.com/ccheever/exact2/bf253d4fac986c8074d44a9ed818ebe417d7a443/realinput-1011c/b1-2set/pairs/G1-8b-pair.png)) | A terminal glyph (`>_`); a popup button "Terminal excerpt, Terminal 2 lines 1-738. Show details"; the Message area's value reads the chip's label ("Terminal 2 lines 1-738 ") |
| G5-D4 | Under VoiceOver, VO-Right from the Message area skips the skill chip and, inside the text, reads the raw `$frontend-design`; the chip (an `AXButton` child of the text view) is reachable only through the Item Chooser ([G5-D4-vo-text-clone.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/G5-D4-vo-text-clone.png)) | The chip is its own element in the editor (ContextChipPopover's trigger, "Skill <label>. Show details"), read by its label in the text; VO-Space opens its details |
| DCP-Gv | VoiceOver reads the prompt's citations as raw Markdown link text (`[Assistant quote](t3-citation://…)`) ([DCP-Gv-captions.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/DCP-Gv-captions.png)) | The citation reads as its chip: the link "View cited assistant text: <label>" and its comment button |
| G5-D1 | VoiceOver's order puts the composer's "Message" between the context window popover's description and its bar ([G5-D1-caption-bar-pair.png](https://raw.githubusercontent.com/ccheever/exact2/aef83daaf9c71586e0d8c17f4c7b2ad0d98cd54c/realinput-1011c/b6-voiceover/pairs/G5-D1-caption-bar-pair.png)) | "Context Window, and 4 more items, dialog": the popover's items stay together as one dialog |
| X30-ATTACH | `EXACT2-GAPS.md` X30 and STATUS §3 still say the any-type file input waits for an owner ruling on #116 | Main admitted it on 2026-10-11 (`3657d330a`, in the T3 tip): correct the cells; optionally move `T3ComposerAttach`'s NSOpenPanel to `input type=file multiple` with `showPicker`, only if a drive shows identical behaviour |

## Steps

1. Reference first: `ComposerPromptEditorTiptap.tsx` (the chips' node views), `contextChipParts.tsx`,
   `chat/TerminalContextInlineChip.tsx`, `composerContextPresentation.tsx`, `contextPresentationRegistry.ts`,
   `chat/AssistantCitationChip.tsx`, `chat/ContextWindowMeter.tsx`, `ui/popover.tsx`; `realinput-1011c`'s AX reads
   (b1-2set/g18, b6-voiceover, b6x-voiceover).
2. Before: the T3 tip (`609f392d5`) built in this worktree before any change, driven on lane
   `composer-chips-accessibility-before` (a prompt holding a terminal reference, a skill, a file mention and a citation;
   `screenshot … window`, `tree --ax composer`).
3. G1-8: the terminal glyph; the chip an accessibility popup button with the reference's name; its press (mouse and
   VO-Space) opens the reference's details (ContextChipPopover: the terminal's name, its lines, the captured output).
4. G5-D4, DCP-Gv: the text view's accessibility text reads each chip as its label (the hidden Markdown source and
   markers left out), the chips are its accessibility children in text order with the reference's roles and names,
   exposed (not ignored) so a navigation-order reader reaches them.
5. G5-D1: the context window popover is a dialog in the accessibility tree, as the reference's PopoverPopup.
6. X30-ATTACH: the record cells.
7. Tests (Bun, AppKit XCTest over the editor's accessibility); one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G1-8 | reference comparison; XCTest (role, label, press, glyph); agent drive (`tree --ax`, `screenshot … window`) | images and AX text records |
| G5-D4 | XCTest over the text view's accessibility (children in navigation order, label, press; the text read as labels); agent drive | AX text records |
| DCP-Gv | the same mechanism and tests, a citation in the prompt | AX text records |
| G5-D1 | a one-file probe of the popover's markup (`tree --ax` order), before and after; the app's contract | AX text records |
| X30-ATTACH | the cells read against main's `3657d330a` | text |
| G1-8, G5-D4, DCP-Gv, G5-D1 under VoiceOver | real-input steps for the final session | — |

## Reference

T3 Code `1e2ecbd975` (MIT), `target/t3-ref/src-1e2ecbd975/apps/web/src/components/`:

- `ComposerPromptEditorTiptap.tsx`: ComposerMentionNodeView (a button "Preview <path>"), ComposerSkillNodeView
  (ContextChipPopover, `accessibleLabel` "Skill <label>"), ComposerCitationNodeView (AssistantCitationChip `composer`),
  ComposerContextReferenceNodeView (the presentation registry).
- `contextChipParts.tsx`: ContextChipPopover (the chip is the PopoverTrigger, `aria-label` "<accessibleLabel>. Show
  details"; Chromium serves it as `AXPopUpButton`), ContextChipShell, UnresolvedChip ("Unavailable context, <label>",
  CircleDashedIcon).
- `chat/TerminalContextInlineChip.tsx`: TerminalIcon; "Terminal excerpt, <label>" (popover with the terminal's name,
  "Line N" / "Lines a–b" and the captured output); expired: "Terminal excerpt, <label>, expired".
- `composerContextPresentation.tsx`, `contextPresentationRegistry.ts`: each kind's name and details mode.
- `chat/AssistantCitationChip.tsx`: the composer's source link "View cited assistant text: <label>" and the comment
  button ("Add comment to citation" / "Edit citation comment").
- `chat/ContextWindowMeter.tsx`, `ui/popover.tsx`: the meter's PopoverPopup (a dialog).
- `realinput-1011c` reads: `b1-2set/g18/{ref,clone}.txt`, `b6-voiceover/G5-D{1,4}-ax-{clone,ref}.txt`,
  `b6x-voiceover/DCP-Gv-ax.txt`, and the session notes `00-b1-notes.txt`, `00-b6-notes.txt`, `00-b6x-notes.txt`.

## Cause and fix

**G1-8, G5-D4, DCP-Gv: one mechanism.** The composer is Exact's `TextArea` (an NSTextView) holding the prompt's Markdown
source; the chips are that source drawn over by `T3ComposerStyler` (X20, #276: a permanent declared difference). AppKit
therefore served the source as the text (`[Terminal 2 lines 1-738](t3-context://…)`, `$frontend-design`,
`[Assistant quote](t3-citation://…)`), and the module's chip buttons were the skill chips and the citations' comment
buttons only, `AXButton`s. Fix, in `T3ComposerAccessibility.swift`:

- Every chip is an accessibility element of the text view, in text order, with the role its node view has in
  T3 Code, as Chromium serves them (the reference's own composer, read over CDP with the same draft): a terminal excerpt
  with text `AXPopUpButton` "Terminal excerpt, <label>. Show details" (expired: "…, expired"); a skill `AXPopUpButton`
  "Skill <label>. Show details"; a file mention `AXButton` "Preview <path>"; a citation its source link `AXLink` "View
  cited assistant text: <label>" and its comment button `AXPopUpButton` "Add comment to citation" / "Edit citation
  comment"; the context kinds as composerContextPresentation names them (thread link, file, image and video
  attachments, review comment, preview annotation, pull request); a reference with no record "Unavailable context,
  <label>". AXPress is offered where the clone builds the press (a skill's and a terminal excerpt's details, a
  citation's comment).
- The text view's accessibility text is a projection of the source: each chip reads as its label, the hidden Markdown
  markers (`**`, backticks, task markers) are left out, a citation's run carries its link element as AXLink, and every
  range (selection, AXSelectedTextRange set by VoiceOver, bounds, lines, index and style ranges, position) maps to the
  source; a chip is one unit (any index in it names its whole label, its bounds are its pill). The text view is
  Exact's and the hatch cannot subclass it, so these answers are added to its class at run time (the typed
  NSAccessibility accessors, which AppKit serves standard attributes from, and the attribute-name methods); each passes
  to the class's own answer for a text view with no projection (no chip, no hidden marker, a composition open, or the
  text mid-edit), so every other text area answers as before.
- Found while proving it: AppKit's server reads a text view's set children through the typed accessor
  (`realinput-1011c` read the skill button out of process) while NSTextView's `accessibilityAttributeValue(.children)`
  does not return them; and the agent's `tree --ax` joins an element under a native text view to no Exact view
  (`AgentAccessibility.axOwner`), so only an unscoped read lists the chips (a framework detail of the agent, not
  filed: the coordinator's call).

**G1-8, the glyph and the press.** `T3ComposerIcons` had no terminal glyph: a terminal excerpt fell to the default
`message-square-text`. It now draws lucide's TerminalIcon. A terminal excerpt that holds text is ContextChipPopover's
trigger in T3 Code: a press (pointer, or AXPress from VoiceOver) now opens its details in the window's chip popover layer
(`composer-chip-popover.contract`, the terminal's name, "Line N" / "Lines a–b", the captured output in a 20rem scroller),
from the draft's terminal context record (`composer-chip-popover.ts` `terminalPopover`); its hover tooltip (the raw
output) is gone, as the reference shows none. A reference with no record drew as an expired excerpt (`composer-editor.ts`
gave every terminal reference an entry); it is now composerContextPresentation's UnresolvedContextChip: CircleDashedIcon
on the neutral chip, "Unavailable context, <label>", "This context is no longer available…".

**G5-D1.** The context window meter's details were `ComposerMenus`' column with `role="menu"` and the fallback label
"Runtime mode". As a menu the column was no accessibility element, so its items were the window's children, which AppKit
orders by position: the composer's Message area, under the popover, fell between the note and the bar (reproduced on a
one-file probe of the markup). The details are now an unnamed `role="dialog"` (ContextWindowMeter's PopoverPopup), one
group VoiceOver keeps together; the meter says `aria-haspopup="dialog"`.

**X30-ATTACH.** `EXACT2-GAPS.md` X30 (the table row, the issue row, the detail line) and STATUS §3's bucket 5 now name
main's `3657d330a` (any-type file input admitted 2026-10-11, adopted in round 11 by `6b6446e97`). `T3ComposerAttach`'s
NSOpenPanel is kept: it also transcodes a picked HEIC to PNG natively (codecs stay deferred), and the optional move to
`input type="file" multiple` needs its own drive comparing the two, which this task's one drive did not cover. X20's row
names the chips' accessibility answers as a kept workaround.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| G1-8 | pass in module/agent checks; real-input and send checks open (module render on the tip's and this branch's modules; AppKit tests; agent drive; reference over CDP with the same draft): before, a speech bubble on a terminal excerpt and no element; after, TerminalIcon, an `AXPopUpButton` "Terminal excerpt, Terminal 2 lines 1-6. Show details" whose AXPress and pointer press open the details (the terminal's name, "Lines 1–6", the output) as the reference's ContextChipPopover; a reference with no record is the reference's CircleDashed "Unavailable context". The send half: real input (step 1) | [glyph](https://raw.githubusercontent.com/ccheever/exact2/c7eea9e2e683f5247b45db1b9a33b42d41872346/composer-chips-accessibility/g18-glyph.png), [details](https://raw.githubusercontent.com/ccheever/exact2/563f79e3d1310dad07d113728123e62a5dfe4d36/composer-chips-accessibility/g18-details.png), [record](https://raw.githubusercontent.com/ccheever/exact2/c1370b7317340dc55d6af696b7b62e5f1ef2e79d/composer-chips-accessibility/cca-record.txt) |
| G5-D4 | pass in process and in the agent's tree; VoiceOver open (VoiceOver's half: real input, step 2): before, the text read `$frontend-design`; after, the Message area reads "Use Frontend Design on …" and the chip is an `AXPopUpButton` "Skill Frontend Design. Show details" child in navigation order whose AXPress opens its details | [drive](https://raw.githubusercontent.com/ccheever/exact2/09b12702e59215ac81450bb4097e1ba090229595/composer-chips-accessibility/chips-drive.png), [record](https://raw.githubusercontent.com/ccheever/exact2/c1370b7317340dc55d6af696b7b62e5f1ef2e79d/composer-chips-accessibility/cca-record.txt) |
| DCP-Gv | pass in process and in the agent's tree; VoiceOver open (VoiceOver: step 3): before, the text read `[Assistant quote](t3-citation://…)`; after, "… with Fixture answer now", the citation an `AXLink` "View cited assistant text: Fixture answer" and its `AXPopUpButton` "Add comment to citation", as the reference's composer | [drive](https://raw.githubusercontent.com/ccheever/exact2/09b12702e59215ac81450bb4097e1ba090229595/composer-chips-accessibility/chips-drive.png), [record](https://raw.githubusercontent.com/ccheever/exact2/c1370b7317340dc55d6af696b7b62e5f1ef2e79d/composer-chips-accessibility/cca-record.txt) |
| G5-D1 | pass in the AppKit markup probe; VoiceOver open (AppKit's navigation order; VoiceOver: step 4): before, the popover's texts, bar and note were window children ordered by position with "Message" between the note and "Compact context"; after, one `AXApplicationDialog` group holding them in order | [record](https://raw.githubusercontent.com/ccheever/exact2/c1370b7317340dc55d6af696b7b62e5f1ef2e79d/composer-chips-accessibility/cca-record.txt) §4 |
| X30-ATTACH | pass: the cells name `3657d330a`; the NSOpenPanel move not made (above) | this record |
| VoiceOver rows | implemented, not verified (the Mac's screen is locked; out-of-process AX reads of windows answer `AXApplication` under the lock): Real-input batch steps below | — |

Drives (agent mode, 1280×840, lane `composer-chips-accessibility`, clone server 16442): before on this worktree's build
of the T3 tip `609f392d5` before any change (lane `…-before`); after on this branch's bundle (`d5727019d`'s code), the
same steps (type the prompt below into the composer, `screenshot … window`, `tree --ax composer`), run once and retried
once with an unscoped `tree --ax` (the scoped read drops elements under a native text view). The terminal excerpt's
record cannot be made by the agent (choosing Add to chat is the AppKit tests'), so G1-8's resolved chip is a render of
the module on a real text view (tip modules, branch modules) and its details a one-file probe of `ChipPopoverLayer` on
the clone's host binary. Reference: T3 Code `1e2ecbd975` on 16440/16441, the same draft written to its
`t3code:composer-drafts:v1` store (with a terminal record and a citation href its parser accepts), read over CDP.
Prompt: `[Terminal 1 lines 1-2](t3-context://v1/terminal/terminal_lane1) Use $frontend-design on [notes.md](notes.md) with
[Assistant quote](t3-citation://v1/thread-1/message-1?text=Fixture+answer) now`.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) and the reference
T3 Code (Alpha) lane app. Turn VoiceOver on (⌘F5) only for these steps and off after. Never send a message to a real
provider.

1. **G1-8.** In both apps, a thread with the drawer's terminal: run `printf 'one\ntwo\nthree\n'`, drag-select the three
   lines, Add to chat. Expected in both: the chip "Terminal 1 lines …" with the `>_` glyph; VO on the Message area reads
   the chip's label ("Terminal 1 lines 2-4 …"), not a link; VO-⇧-↓ into the text and VO-→ through it, or VO-→ from the
   Message area: record whether VO lands on "Terminal excerpt, Terminal 1 lines …. Show details, pop up button" in each
   app (the reference's half was never observed); VO-Space on it opens the details (the terminal's name, "Lines a–b", the
   output) in both; Escape closes them. A plain click on the chip opens them too. The send half (G1-8 c): only against the
   lane's fake provider if the session has one, else not run (no provider messages).
2. **G5-D4.** Clone, a new thread, type `$frontend-design now ` (no Return). VO reads "Frontend Design now" in the Message
   area, not `$frontend-design`; reach the chip as in step 1 ("Skill Frontend Design. Show details, pop up button");
   VO-Space opens its details; Escape. The reference's half needs a skill its lane lists (else it shows plain text).
3. **DCP-Gv.** Clone, an answer in the fixture thread: select a phrase, Cite. VO on the Message area reads the quote (or
   the comment), not `[Assistant quote](t3-citation://…)`; VO lands on "View cited assistant text: …, link" and "Add
   comment to citation, pop up button" (VO-Space opens the comment; Escape).
4. **G5-D1.** Clone, Settings › Context window indicator (legacy) on, a thread with token usage, the ring's popover
   open (focus on Compact context): VO-Left goes note → bar → "26% · …" with no "Message" between, and VO names the group
   "Context Window, and N more items, dialog" as the reference does.

## Tests

- `macos/tests/composer/chipaccessibility.swift` (AppKit, 13): the text read as labels both ways (typed accessors and
  attribute names: AXValue, characters, string, attributed string with the citation's AXLink element, a chip one unit,
  lines); ranges mapped both ways (selection read, AXSelectedTextRange set, bounds on the pill, range at a point); the
  chips as elements in text order with the reference's roles and names, in navigation order, on their pills; the
  terminal excerpt's AXPress and pointer press open its details (`kind` "terminal", the context id), no tooltip; an
  expired excerpt a plain shell; a reference with no record "Unavailable context"; the terminal glyph; a plain prompt and
  another text view keep their own answers; the read text follows an edit. `chippress.swift` and `citationpress.swift`
  take the reference's roles and names (pop-up buttons, the citation's link). 78 tests in the composer binary.
  The repair adds unchanged-draft skill/rich-text updates, PR state/title names, typed selection/shared ranges and RTF, and an offscreen fill check in light and dark appearance.
- `composer-chips-accessibility.test.ts` (Bun, 4): the terminal excerpt's popover (name, lines, output, rows; no record,
  another kind or no text shows nothing); a terminal reference with no record gets no context entry; the native PR
  context payload preserves the state and title, including tabs and Unicode.
  `composer-chip-popover.test.ts` takes the shape's two new fields.
- `macos/src/composer_accessibility_tests.rs` runs the compiled app plan with fixture replies. It checks the context
  meter/dialog semantics and containment, and the terminal popover's title, name, lines and output. These replace the
  four source-string checks removed from `composer-chips-accessibility.test.ts`.

## Checks

Historical checks before this review repair (`9d840b137`, code of `d5727019d`; base `609f392d5`):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4863 pass, 1 skip, 0 fail (329 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,453 lines; 120,818 nodes, 33,310,902 bytes |
| `cargo test -p t3-code-macos --lib` | 0 | 25 passed |
| AppKit `macos/tests/composer` (README recipe) | 0 | 74 tests, 0 failures |
| AppKit `sidebar`, `r9-input`, `r5-composer` | 0 | `r9-input`'s first run failed `testClickFocusReachesTheNodeOnce` (4 assertions: the focus bridge's async first-responder observation and its 20 ms tick); three reruns of the same binary pass, and the binary built from the tip's modules passes |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once on the tip (before), then on `f802b1a3c`'s code and again on `d5727019d`'s (the unresolved reference) before the live drive |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed, 0 failed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

## Not done / not verified

- VoiceOver on the four rows (Real-input batch steps 1-4): the Mac's screen is locked, so VoiceOver cannot run and
  another process reads every window as `AXApplication`; AppKit's answers were read in process (the typed accessors
  AppKit serves standard attributes from, and the attribute names), and in the live app through the agent's `tree --ax`.
  Whether VO-Right from the Message area lands on a chip is VoiceOver's own navigation of a text area: the chips are now
  its children in navigation order, as the reference's, but neither app's behaviour there was ever observed.
- G1-8's send half: it sends a message, which the rules forbid here (step 1 says when it may run).
- X30-ATTACH's optional move of `T3ComposerAttach` to `input type="file" multiple`: not made; it needs a drive comparing
  the NSOpenPanel with the host's picker (multiple selection, the HEIC transcode the module keeps), outside this task's
  one drive.

## Seen while building, not a finding row

- Composer chips whose press the clone never built: in T3 Code a mention opens the file's preview, a file, video or
  image attachment opens or expands, a thread chip opens the thread, a citation's source link jumps to the cited text, a
  pull request chip opens it, and review comment and preview annotation chips open popovers. The clone's click places
  the caret; their elements now carry the reference's names and offer no AXPress.
- An expired terminal excerpt (a record with no text): the reference draws ContextChip's invalid state (destructive
  colours, solid border); the clone keeps its dashed look.
- The agent's `tree --ax <target>` drops an element whose accessibility parent is a native view inside a node
  (`AgentAccessibility.axOwner` joins it to no view), so a scoped read of the composer lists no chip; the unscoped read
  does (framework, not filed).
- Exact serves a `text` node's `aria-label` as its value too: the details' output reads "Captured terminal output" as its
  value, where Chromium's `pre` has that name and the output as its text (the timeline's terminal popover is the same).

## Review repair, 2026-10-14

The repair starts at draft head `168ce6a0c`. The two saved independent reviews identified a stale skill label,
missing pull-request state/title in the chip name, a light-only unavailable-chip fill, source-string-only popover
checks, and acceptance wording that treated the unrun VoiceOver checks as passes. Typed selection ranges and RTF
already use the projection, but the new test found SharedCharacterRange still reporting source length 205 instead of
the projected length 82. The existing range-wrapper loop now covers that accessor too.

Guidance: implement workflow library `20261005-platforms-v3`, foundations, accessibility, platform and testing topics;
this checkout's `docs/testing.md` and the parent spec's practical core-check direction. Only the macOS app is in scope.
The branch already contains prepared base `609f392d5`. No framework files or shared plan indexes are changed.

- Restyling invalidates the accessibility text projection. Provider skill names and the rich-text preference update
  AXValue and AXStringForRange even when the source and parsed chips stay identical.
- The native context payload retains the PR title. A PR chip now reads `Open Open pull request #12: Fix x`, or the
  matching Draft, Merged, Closed or generic Pull request form, as `contextChipParts.tsx` names it.
- An unavailable chip uses the dark accent fill in dark appearance. An offscreen bitmap check covers both appearances.
- X30's issue row now says #116 is closed; its remaining codec/readback scope is deferred, bucket 6. The coordinator owns
  removing #116 and correcting the count in STATUS bucket 5.
- VoiceOver acceptance stays open. Attachment chip names still use their display labels rather than the record's full
  filename; those attachment names are outside these finding rows and are not claimed as parity.

The app-local accessibility method hooks remain the workaround recorded in X20. They delegate to the host for other
text views. The existing typed range wrappers now include SharedCharacterRange. A future TextArea accessibility API
change requires rechecking this seam.

Regression evidence: before the repair, the expanded AppKit suite ran 77 tests with nine failed assertions in the new
skill/rich-text and PR-name cases; the new Bun PR-payload test failed because its payload contained only `pr-open`.
After the repair, the final AppKit run passed 78 tests and the final Bun run passed 4,860 tests with one skip.
The focused Bun pair passed 14 tests after removing the four source-string checks.
Strict TypeScript checking passed. The added runner tests exercise the actual compiled app plan with fixture resource
replies, covering the context menu's dialog semantics and terminal popover content without a server or source-string
assertions. The Rust app suite passes all 27 tests.

Repair checks (pinned Bun 1.4.2 on PATH; log paths relative to `target/t3-tests/`):

| Check | Result | Log |
| --- | --- | --- |
| AppKit composer, README build recipe | 78 passed, 0 failed | `composer-review-shared-range-swift.log` |
| `bun test examples/t3-code --timeout 60000` | 4,860 passed, 1 skipped | `composer-review-bun-final.log` |
| strict TypeScript, README command | exit 0 | `composer-review-tsc.log` |
| `cargo test -p t3-code-macos --lib` | 27 passed | `composer-review-rust-final.log` |
| `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle` | exit 0, fresh native build | `composer-review-native-final.log` |
| `cargo fmt --all -- --check` | exit 0 | `composer-review-fmt.log` |
| `bun scripts/caps.mjs` after staging | exit 0 | `composer-review-caps.log` |
| `bun scripts/boot.mjs` | exit 0 | `composer-review-boot.log` |
| macOS agent launch, composer edit and unscoped `tree --ax` | exit 0 | `composer-review-agent-edit.jsonl` |

The native drive uses isolated storage `t3-composer-review-534-20261014`, a 1280×840 window, and the app built
from the final source. Its `type composer` operation delivers through the macOS platform. Given
`Use [notes.md](notes.md) with [Assistant quote](t3-citation://v1/thread-1/message-1?text=Fixture+answer) now`,
the Message AX value is `Use notes.md with Fixture answer now`. Its children are `Preview notes.md` (AXButton),
`View cited assistant text: Fixture answer` (AXLink), and `Add comment to citation` (AXPopUpButton), in that order.
The server is disconnected and the drive sends no message. This proves the native edit/projection path, not live
VoiceOver traversal or provider behavior. The first agent launch was refused as stale because source changed during
the earlier native build; the fresh rebuild and drive above passed without overriding the freshness check.

Repository-wide build, test and clippy checks were not repeated for the repair; the coordinator owns those integration
checks. The seven changed behavior/test files are bound by source fingerprint
`c157e0b26c513dcb9c371c667184cdc6573a2bfa0ebd15409ec1ece920403159` in `composer-review-source.json`.

Local logs are under `target/t3-tests/composer-review-*`; they are not published evidence. The reference source used
for PR names is the pinned T3 Code `contextChipParts.tsx` and `lib/composerContextRecords.ts`, inspected read-only.

## Delivery

Draft PR [#534](https://github.com/ccheever/exact2/pull/534) into `feat(example)/t3-code`; the coordinator reviews and merges.
The earlier gate results above belong to `9d840b137` (code of `d5727019d`); repair checks are listed separately.
This worker does not push, publish the PR update, or merge.

## Next action

Implementation is ready for independent verification. The coordinator owns integration checks, the PR update and
final acceptance; the prior implementation verification remains stale until that handoff completes.
Real-input batch steps 1-4 remain open acceptance rows.

## Coordinator publication, 2026-10-14

Draft PR [#534](https://github.com/ccheever/exact2/pull/534) was read back at `fd8c6541ef841649868e3559cf150faf37525bee`, based on `feat(example)/t3-code`. Projection, ranges and labels reviewed; native AX proof passed; VoiceOver and terminal-send remain open. The combined repository gates and app drive are pending. This local integration bookkeeping does not close the task or replace the earlier verification revision.

### Remote delivery, 2026-10-14

GitHub reports this PR merged at 03:04 UTC into `feat(example)/t3-code`, published as
first-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`. This records delivery
only; the task's remaining acceptance and real-input qualifications above still apply.
