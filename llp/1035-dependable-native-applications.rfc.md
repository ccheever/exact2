# LLP 1035: Dependable native applications

**Type:** RFC
**Status:** Draft r2 (r1 by Codex 2026-09-09, the prioritized proposal Charlie asked for after the Messages exercise; r2 the same day — reviewed against the code, the Messages record and the staged spec amendments, then expanded: every child now carries numbered decisions, a first slice with acceptance, the amendments it makes at acceptance, and the questions only Charlie can answer. r1's claims that did not survive the check are corrected in §7.) **Landed 2026-09-09, uncommitted:** 1035.000 slice 1 (the eleven inherited rows as schema metadata, runs and editors measuring with computed rows on every host, kernel-owned invalidation), 1035.002 slice 1 (`layout <target>` on every carrier), 1035.003's web and macOS half (a contact's phases as forms of `tap`, delivery classes on every input reply, iOS and Linux answering `unsupported`). Not started: 1035.001 slice 1, 1035.003's Simulator gate, 1035.004, 1035.005. Each child's Status line says exactly what it holds.
**Systems:** Kernel, Contract, Apple host, Web host, Linux host, Agent API, Authoring
**Author:** Charlie Cheever / Codex (r1); Claude (Fable 5.1) for Charlie Cheever (r2)
**Implementer:** Claude (Fable 5.1), from 2026-09-09, for the first slice of each child in the order of §3 (Charlie, 2026-09-09: "go ahead and implement once you are happy with them"). Codex continues the Messages repairs in the same tree. A later slice gets its own implementer and date when it is selected (`rules/RULES.md` §Scope).
**Date:** 2026-09-09
**Revised:** 2026-09-09 (r2)
**Related:** LLP 1000, 1001 §1/§5/§6, 1005 §3, 1006 §2/§8, 1007 §1/§4, 1008 §2/§3/§5/§9, 1011, 1012 §1/§6/§8, 1017 §8, 1017.000, 1021, 1030 stage 1 (the asset row), 1031 D9/D10, 1033, 1034; the children LLP 1035.000–1035.005; `apps/messages/README.md` (the application record); `rules/NOT-DOING.md` (binds; §5 names the two lines this program asks to amend)

## Summary

Make building a demanding native application a dependable use of Exact2. An
application describes its content and its intent; Exact supplies coherent
semantics, native ownership of the machinery the platform already owns, and
enough evidence to diagnose the painted result without an LLDB session, a
temporary log line, or a disposable input script.

The Messages exercise — a local-only iPhone chat app driven to the point of
comparing its pixels and gestures with MobileSMS on an iPhone 17 / iOS 26.5
simulator — repaired a dozen individual defects (§1) and left the general
problem standing: the supported contract and its inspection surface are
thinner than the behavior a real application needs, and the happy path
passing proves neither. Six children name the six improvements, in the
order that pays back fastest:

| Priority | Desired outcome | Child | First slice |
|---|---|---|---|
| 1 | Rely on the CSS contract across every native boundary | [1035.000 — Dependable host semantics](1035.000-dependable-host-semantics.rfc.md) | text rows inherit into runs the way `color` now does; `inherited` becomes schema metadata |
| 2 | Express navigation, presentation and editing intent once | [1035.001 — Native interaction ownership](1035.001-native-interaction-ownership.rfc.md) | the lifecycle of the existing projection written down and held by fixtures; then one owed defect |
| 3 | Inspect the authored and the painted result together | [1035.002 — Native result inspection](1035.002-native-result-inspection.rfc.md) | `layout <node>` explains a property's source and names its coordinate spaces; `state` reports focus, keyboard, route |
| 4 | Reproduce a real gesture, including its interrupted states | [1035.003 — Reproducible native gestures](1035.003-reproducible-native-gestures.rfc.md) | a held contact as forms of `tap`, on the carriers that can hold one; the rest report unsupported |
| 5 | Reuse a small set of native visual affordances | [1035.004 — Native visual affordances](1035.004-native-visual-affordances.rfc.md) | six symbol roles as an `image` source, native where the platform has them |
| 6 | Keep large Contract components navigable and editable | [1035.005 — Contract authoring ergonomics](1035.005-contract-authoring-ergonomics.rfc.md) | `contract fmt` and `contract symbols` on the existing parser |

The children refine this program and propose amendments to the LLPs that
own each system. They do not replace the kernel, host, agent or language
specifications with six competing ones: at acceptance each child's "what
this amends" section lands in LLP 1001, 1007, 1008, 1012 or 1006, and the
child stays the record of why.

## 1. What the Messages exercise established

The [application record](../apps/messages/README.md) is the evidence; this
section is its index, item by item, so a reader can see which child each
finding belongs to and which findings are repaired, stopped, or open.

**Repaired in the tree today** (Codex's staged work of 2026-09-09; the
spec amendments are staged beside it):

- **Inherited colour crossed no native boundary.** The root declared
  `color="light-dark(#000000, #ffffff)"`; the native inbox heading and the
  sender names, which declare no colour of their own, painted black on a
  cold dark launch. The browser inherited correctly. `NodeRef::text_color()`
  now walks the logical ancestors (LLP 1001 §6), the Apple style projection
  resends a descendant when its computed colour changes, and the host
  regression covers an ancestor update, reparenting and a cleared override
  (LLP 1008 §2). → 1035.000.
- **Whole-point rounding of fractional frames.** Taffy's rounding was
  on; a quarter-point inset vanished and a half-point height edit moved a
  sibling by a whole point or not at all. Rounding is off in the layout
  tree, rebuilds and rehydration included (LLP 1001 §5); Apple text
  measurement now preserves an authored fractional line height, while the
  intrinsic width, a `normal` paragraph height and painted baselines still
  round (removing the ceilings moved Caltrain's pinned scroll extent), and a
  UILabel/CoreText/WebKit comparison found no single baseline adjustment
  that matches both. → 1035.000.
- **A keyboard change inside an older batch.** Removing the focused sheet
  announced a keyboard resize synchronously during a presenter batch, whose
  remaining frames then overwrote the restored inbox height. The change
  now waits for the batch to end and keeps the keyboard's duration and curve
  (LLP 1008 §9). → 1035.001.
- **A sheet mistaken for a pop.** The viewport freeze that keeps a composer
  beside a sideways-moving keyboard during an interactive pop was applied to
  the sheet's vertical dismissal, freezing the composer under the keyboard.
  Modal transitions are excluded from it; the sheet uses its own keyboard
  guide (LLP 1008 §9). → 1035.001.
- **Sheet presentation needed controller containment and a live source.** A
  focus command arriving before the sheet's field was mounted had to wait;
  a screenshot of the source route went stale when appearance changed
  behind the sheet. The source controller and its views now stay live in the
  presenting surface, inert to input and accessibility, with deferred
  geometry replayed on close (LLP 1008 §9). → 1035.001.
- **A 27-point strip from editor traits.** The recipient keyboard carried
  UIKit's empty prediction strip. HTML's `spellcheck` hint, resolved through
  logical ancestors in the kernel, removed it without an app-side inset:
  the keyboard is 308 points high with its top at screen y=566, as native
  Messages on the same simulator (LLP 1001 §1, 1006 §2, 1008 §5). → 1035.001.
- **Host-owned sheet dimming.** The app painted a backdrop inside UIKit's
  sheet surface and the two disagreed at the corner. UIKit now owns the
  dimming; the 160×124-pixel light-mode crop matches the native capture
  exactly (LLP 1008 §9). → 1035.004.
- **Reading position on contact-details Back.** A later bounded repair used
  `layout <node>` to isolate the 160-point jump: the inactive conversation's
  taller viewport clamped its offset, then end-following misread the clamp.
  The iOS host retains the unpinned intent across that temporary clamp and
  compares UIKit's actual stored offset after quantization. Ordinary-launch
  button Back and cancelled/completed swipes preserve the transcript and
  keyboard; `apps/messages/README.md` records the evidence. → 1035.001 D7.

**Earlier investigations stopped at the three-round limit**
(`rules/RULES.md` §Loop shape; the follow-up below updates their status):

- Physical taps in the inbox row's *leading* action area appeared not to
  reach the authored control, although agent activation did. **Revalidated
  2026-09-10:** foreground-guarded physical Read/Unread taps now pass, including
  an ordinary launch, reversal, row switching and draft/history retention.
  No runtime repair was needed; the old input's delivery remains unknown.
  Evidence: `apps/messages/README.md`, `/tmp/messages-leading-foreground/`.
  → 1035.003 (the instrument).
- A bubble-bound badge wrapper made physical badge taps regress
  intermittently; reverted. → 1035.003 first — the failure was only ever
  seen through the disposable input path.

**Open, and the reason the program exists:**

- The agent's `tap` on iOS is a hit-test and an activation, never a UIKit
  touch (LLP 1008 §9's declared deviation; LLP 1012 §1). Every "physical"
  claim in the record came from a temporary pointer helper, manually
  translated Simulator desktop coordinates, and a second offset once a
  modal was up. Two normal-mode failures are unexplained: an outside-dismiss
  tap at (390, 170) missed once and passed under the agent; the first inbox
  drive of a session missed its initial gesture and another lost its row,
  then passed. → 1035.003.
- Nothing reports *why* a value won. Diagnosing the heading colour meant
  reconstructing the kernel's rows and the presenter's dictionary in LLDB;
  diagnosing the sheet's offset meant combining a kernel frame with a
  window offset by hand. → 1035.002.
- Native chrome the app cannot reach: an 80-point drag moves UIKit's
  large-title table 62 points while its top inset collapses 168 → 116; a
  bare scroll view — and Exact — move 36.67. A public scroll-edge fixture
  produced the soft header edge with a native label and not with a
  custom-painted text view. → 1035.001 and 1035.004.
- 35 `clip-path` attributes — 20 inline `path()` literals, 12 distinct,
  2,251 bytes, plus 15 through five derives — and rotated-box chevrons
  stand in for Back, Close, Compose, Add, Microphone and Send. A pixel
  comparison overturned two apparent rendering defects in image previews
  that a screenshot preview had suggested. → 1035.004, and the "saved
  pixels, not the preview" rule in 1035.000.
- `NewMessageSheet` declares 16 props (7 values, 9 actions) bound at one
  522-character call site; the two components span 726 lines, 88 of them
  over 200 characters, the two bubble declarations 829 and 841; the named
  styles, `fn` and `class=` LLP 1017 landed are unused there. → 1035.005.
- A shared *product* difference that is not a semantic defect: the long
  native bubble is ~262 points wide, Exact's 277 on the iPhone and 277.5 in
  the browser, all three wrapping to the same three lines. Native Messages
  shrinks a balloon to its wrapped ink; CSS does not. → 1035.000 §3 names it
  as out of scope, not a kernel change.
- Arbitration a platform owns: a back swipe over the horizontal timestamp
  scroll area goes to that scroll view. → 1035.001 records it as UIKit's
  answer unless a bounded fix appears.

## 2. Architectural position

Keep the existing division of work, and write it down once so no child
re-argues it:

1. **CSS and the browser define the shared semantics** — property names,
   defaults, value vocabularies, inheritance, behavior (`rules/RULES.md`
   §Scope). A native host that disagrees with a bare `<div>` has a bug;
   an unavoidable deviation is declared in LLP 1001 §1 with its reason.
2. **The kernel owns the logical tree and native layout.** UIKit and AppKit
   containment — a navigation controller, a sheet, a scroll view, an
   embedded session — changes mounting and coordinates, never CSS ancestry,
   and never becomes a second source of inherited values or application
   state.
3. **Platform controllers own their machinery.** Push/pop animation,
   interactive pop recognition, sheet presentation, keyboard guides, editor
   selection and composition, scroll arbitration. Exact declares intent
   (which route, which sheet, whether dismissal is permitted) and receives
   outcomes. No gesture arena, no transition-progress graph, no Core
   Animation executor (`rules/NOT-DOING.md` §Motion).
4. **The driver reports what it actually exercised and observed.** An
   activation is not a touch; a model frame is not a presentation frame; a
   preview is not the saved pixels. Every reply says which it is.

Two oracles, for two questions. **For a CSS semantic, the browser** — the
parity corpus already holds layout and motion to it and 1035.000 extends
that to inheritance, clipping and text geometry. **For a native
presentation, native Messages on a pinned configuration** — the record's
comparisons are against MobileSMS on the iPhone 17 / iOS 26.5 simulator,
and stay so until Charlie picks another. Neither oracle promises that UIKit
pixels equal browser pixels or that an iOS release has one permanent
appearance: comparisons declare the fonts, device scale and tolerance
first (1035.000 D8), and preserve the product's meaning when the platform's
appearance changes.

## 3. The program: six children, one order, the interlocks

The order is by leverage — what each child unblocks for the next — and it
is a *starting* order, not a rule to work blind:

- **1035.000 first** because a semantic defect hides under every other
  finding: a colour that does not inherit looks like a theming bug, a
  rounded frame looks like a layout bug, a ceiled paragraph looks like a
  bubble-sizing bug. Its first slice needs nothing from the others.
- **1035.002 and 1035.003 are the instruments**, and they are built to the
  minimum needed to verify 000 and 001 *while those proceed*: `layout
  <node>` explaining an inherited value is the way to prove 000's slice
  without LLDB; a held contact is the only way to prove 001's lifecycle
  rules at the held, cancelled and completed positions. Priority order does
  not mean every semantic issue is solved before an instrument exists.
- **1035.001 consolidates** what Messages already exercised — the
  projection exists, its rules do not — and then takes one owed defect.
  It reads 000's focus and appearance rows and is verified through 002 and
  003.
- **1035.004 and 1035.005 remove app-owned weight** — geometry the platform
  has, declarations the compiler can format and navigate. 005 shares 002's
  node-to-declaration map; 004 replaces the paths 005 would otherwise have
  to format.

Interlocks, so nothing is built twice: the node/session/incarnation
identity that 002 tags reads with is the identity 003 resolves a target
against; the coordinate-space names 002 defines are the spaces 003 injects
in; the "authored presence vs computed value" distinction 000 stores is what
002 reports; the plan-node site 002 returns is what 005's map is keyed by;
the editor-trait inheritance 000 generalizes is what 001 verifies across
handoffs; the material and symbol mappings 004 adds are inspected through
002's `applied` field.

## 4. Sequence and evidence

Start with one failing semantic boundary and its browser reproduction.
Finish the smallest *complete* repair — creation, update, reversal,
removal, reload — before widening the property surface. Consolidate the
navigation and editing behavior Messages already exercised. Build the
inspection and input support needed to verify those repairs as they land.

**The first demonstration** is one local Messages flow, driven end to end
with the tools these children add and nothing disposable: open a
conversation, type a draft, open forwarding, drag the sheet, hold it,
inspect it, reverse it, close it, cancel and then complete Back, change the
appearance, and inspect the resulting nodes — with the held pixels saved,
the source state read, and each input's delivery mechanism named in the
reply. **Then a second consumer per child**, because a fix that only
Messages needs belongs to Messages: Caltrain for semantics and inspection
(it has real lists, search, navigation and theming), Fieldnotes for editing
and focus (a multiline editor on web and Apple, LLP 1033/NOT-DOING
2026-09-07), the embedded two-session host (LLP 1031 D10) for ownership
across sessions, the Markdown readers for text-row inheritance (their
workaround is the QUEUE line of 2026-09-08), and Weird Castle for the
affordances if its wordmark lane wants one.

Each landing updates the owning subsystem LLP and the relevant smoke or
corpus case. The five checks stay five; every check reports all failures;
fix loops stop at three rounds; source files stay under 1,500 lines; the
boot graph does not grow (`rules/RULES.md`). Native comparisons that exceed
the blocking budget run asynchronously. No child proposes a verification
registry, a review loop, a per-PR ledger or a new check category.

## 5. Scope trades

`rules/NOT-DOING.md` binds. The development source map needs an amendment
there; the formatter also changes LLP 1006 §8's explicit exclusion. Both
are proposed here with their scope trades:

| Proposed addition | What it unblocks | The take, and the boundary kept |
|---|---|---|
| Bounded style and host-state detail in the existing `tree`/`layout`/`state` reads (1035.002) | diagnosing the painted native node without LLDB | further bespoke Messages pixel tuning waits behind explaining the existing mismatches; no devtools UI, no ninth operation |
| A development-only map from plan node to declaration span and component call-site chain, emitted by the compiler, read by the driver (1035.002 D6, 1035.005 D3) | jumping from a failing node to its declaration and instantiation site | **amends NOT-DOING §Agent API** ("contract witness / dataflow / source-map / bindings"): the map is admitted, the general witness/dataflow explorer stays out. The take: no new Contract syntax until formatting and navigation have landed (1035.005 §1's order), and the `contract` blocks as executable assertions (LLP 1006 §8) stay off the doing-list |
| Driver-owned pointer sequences and checkpoints as forms of `tap` (1035.003) | verifying a keyboard drag, a sheet reversal and a Back cancellation | replaces the disposable helper and the hand-mapped offsets; further gesture-dependent Messages polish waits behind it. Already inside NOT-DOING: a drag is a form of `tap` (LLP 1012 §1, Charlie 2026-08-29); no recorder, no replay system, no coordinates reach the runner |
| Six symbol roles as an `image` source and the existing material row inspected (1035.004) | replacing the matching standard-control shapes; the app's 35 `clip-path` uses include custom artwork that stays (§3 of the child names the removals) | no new tag (the count stays ~15), no component catalog, no theme framework; Linux paints the declared fallback or a declared gap |
| Native route projection extended with title intent (1035.001 D9) | UIKit's large-title collapse, insets and scroll edge on the inbox | extends LLP 1008 §9's projection; **does not** admit the interactive-navigation model NOT-DOING excludes — UIKit still owns recognition, progress and cancellation |
| Formatting and source navigation through the existing Contract compiler (1035.005 D1/D2) | making long declarations and component action bindings readable and findable | **amends LLP 1006 §8** to admit the formatter; the LSP stays out. The proposed continuation rule is the bounded grammar change needed for formatting; further syntax expansion waits behind these tools |

If implementing a child turns out to need a genuinely excluded capability
beyond these amendments, acceptance must name the precise amendment and its
take. Draft status is not an exception to the rules.

## 6. Decisions — decided (Charlie, 2026-09-10: "do all your recs")

1. The priorities and first slices in §Summary's table stand.
2. **The native input backend:** prove desktop-pointer injection into the
   Simulator window first, with the window-to-device mapping in one place in
   the driver; an XCTest bundle only if the pointer cannot hold a contact or
   cannot run headless on the fleet; a physical iPhone stays "unsupported,
   reported" until an XCTest bundle exists. The ordinary-app launch under the
   socket is opt-in, `EXACT_AGENT_TIMING=platform` (1035.003 D5), never the
   default. The helper this needs is approved as apparatus.
3. **The source-map amendment:** accepted as written in `rules/NOT-DOING.md`
   §Agent API — the development-only map, the driver its only reader; the
   take is no new Contract syntax before formatting and navigation land, and
   `contract` blocks as executable assertions stay off.
4. **Symbol access:** roles only (`image "symbol:<role>"`); the web fallback
   is host glue, not an app asset; a platform name is never admitted without
   a role row; `search` is the seventh role.
5. **The editor-state guarantee** (1035.001 D7): a retained view keeps text,
   selection and composition; a destroyed one keeps nothing the app did not
   save; reveal never moves a sibling scroll container; focus restoration
   stays the app's `focus` command. Title intent reaches UIKit through a
   header-shaped route after a prototype (D9); the back swipe over a
   horizontal scroller is UIKit's answer (D1).
6. **The continuation-line grammar** (1035.005 D1): admitted — continuation
   lines indented deeper than the element and beginning with `name=`. The
   action-prop arity check is a refusal (D2). The `path` asset waits for a
   third app (D5).

## 7. Review notes on r1 (r2, 2026-09-09)

What the r2 check against the code, the Messages record and the staged
spec amendments changed, so the corrections are visible rather than
silent:

- r1's children had no numbered decisions, no implementer, no landing order
  with acceptance, no amendment sections and no questions section. Each
  now has all five, in the shape LLP 1031 and 1034 use.
- 1035.000 r1 said Apple paragraph measurement "still ceils dimensions";
  the final repair preserves explicitly authored fractional line heights.
  Intrinsic widths and `normal` paragraph heights still ceil, and painted
  baselines still round. 1035.000 D6 names all three remaining roundings
  and the UILabel/CoreText/WebKit finding that no single baseline
  adjustment matches both.
- 1035.000 r1 left "how far to generalize inherited style metadata" open
  with no candidate. The QUEUE line of 2026-09-08 — text rows do not
  cascade into inline runs, so the Markdown readers thread the type through
  as props — *is* the next failing property, with two consumers. r2 makes it
  the first slice and proposes `inherited` as schema metadata (D1).
- 1035.001 r1 listed the route vocabulary without its definitions; r2 D1
  restates each from LLP 1001 §1 and 1008 §9 as amended, so the
  consolidation has a baseline to consolidate.
- 1035.002 r1 proposed fields with no request form. r2 gives the wire and
  CLI shape for each, as forms of the existing operations (D1–D4), with the
  identity tags LLP 1012 already returns (`epoch`, `incarnation`) reused
  rather than reinvented.
- 1035.003 r1's protocol sketch had no coordinate-space rule and no
  capability report. r2 defines both, and states which existing carrier
  input is already a real platform event (the web's CDP mouse events,
  macOS's `sendEvent`) and which is not (iOS's activation). r1 spoke of
  "the existing Simulator desktop-pointer experiments" as a starting path;
  the tree and its history hold no such code — the helper was never
  checked in — so the feasibility gate starts from nothing but the
  record's description of it.
- 1035.004 r1 deferred every name to a prototype. r2 keeps that for the
  *access* shape (role vs platform name) but fixes what can be fixed now:
  a symbol is an `image` source, its size and weight follow `font-size` and
  `font-weight`, its colour is `tint_color`, and an unknown role is a
  journal line plus the declared box — so the prototype measures one design
  rather than inventing one.
- 1035.005 r1 said "if safe multiline attributes need a grammar change,
  make that a small explicit proposal"; r2 checks the grammar and makes the
  proposal (D1), because the formatter has nothing to format without it.
  r1 also said "provide formatting through the existing Contract
  CLI/compiler, not a separate parser"; the existing lexer discards
  comments and blank lines, so the formatter needs trivia in the token
  stream first — r2 D1 says so. And r1 assumed an action prop's two sides
  could be diagnosed; today an `action` prop has unknown arity and is
  never checked at the binding — r2 D2 closes that first.
- The umbrella's r1 evidence list was seven bullets; r2 §1 indexes the
  record item by item and marks each as repaired, stopped, or open.

## 8. Working-set change

These seven documents occupy seven of the fifteen `llp/current/` slots.
For this work the links for LLP 1020, 1023, 1026, 1027, 1029, 1030,
1030.000 and 1030.002 left that overlay (staged 2026-09-09). Their
documents and statuses are unchanged; the root map and cross-links keep
their decisions reachable. This focuses the working set on semantics,
authoring, inspection and native application behavior without raising the
cap or claiming those designs are superseded.
