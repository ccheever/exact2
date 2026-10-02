# LLP 1075: Native platform control — James's UIKit problem

**Type:** Explainer
**Status:** Draft
**Systems:** Apple host (UIKit objects, navigation, tab containers, controller lifetime); native modules; ExactKit embedding; Contract and router state; TypeScript and Rust native interop; web presentation
**Author:** Codex for Charlie Cheever; problem reported by James (`ide`)
**Date:** 2026-10-01
**Revised:** 2026-10-01
**Contributors:** Claude (Opus 5.5, then Fable 5.1), at Charlie's request on 2026-10-01: the paragraphs marked *(Claude)* in §2, §6 and §7, and the 1075.002 and 1075.003 rows in §5. They add evidence, questions and bookkeeping, not a recommendation.
**Related:** [1024](1024-native-modules.rfc.md) (native views); [1031](1031-brownfield-embedding.rfc.md) (embedding); [1035.001](1035.001-native-interaction-ownership.rfc.md) (native ownership); [1035.006](1035.006-public-ui-coverage.plan.md) (coverage and extension policy); [1037](1037-dartnative-lessons.research.md) (the navigation-bar lesson); [1038](1038-router.rfc.md) (router); [1059](1059-tab-bar-projection.rfc.md) (today's tab bar); [1067.000](1067.000-one-native-module-or-two.rfc.md) (Swift modules); [1072](1072-building-rows-off-the-main-thread.rfc.md) (runtime ownership)

## 1. The problem

James wants to use the native platform's capabilities without waiting for Exact
to expose each one or maintaining a fork of Exact. He also values sharing
application logic and layout across platforms. His concern is that choosing
Exact may turn an ordinary UIKit task into framework work, with recurring costs
when Exact changes.

This is the common problem statement for independent proposals. It selects no
interop technology, authoring language, controller architecture, or API. Charlie
requested this parent and Codex's two children on 2026-10-01 and intends to have
Claude contribute independent sub-LLPs as well. Those contributions should be
linked here when written; this document attributes no recommendation to Claude.

## 2. Evidence supplied by James

The primary evidence is three conversation screenshots and a patch pasted by
Charlie in the 2026-10-01 discussion. They have not been independently reproduced
in James's app. Short quotations below are from those supplied screenshots.

### 2.1 Native navigation bars require a framework patch

James reports: “I find it necessary to patch Exact to get what I want,” with
large navigation headers on iOS as his latest example. He expects more native
properties to be needed and wants to solve the class of problem.

His supplied patch is headed by commit
`da571303f263c770498c1a84f9a11c5b54963b2b`, with subject beginning
“navigation: a route can ask for UIKit's navigation bar (LLP 1038)”. The patch
names `expo-tuft[bot]` as author and `ide` as co-author and carries the date
2026-10-01. This identifies the supplied artifact; it does not mean that commit
exists in this checkout or that the patch was merged or verified.

The 99-line addition touches three layers:

| File | Change in the supplied patch |
|---|---|
| `kernel/tables/schema.json` | Five string properties: `navigationTitle`, `navigationLargeTitle`, `navigationTrailing`, `navigationTrailingSymbol`, `navigationBackButton`. |
| `contract/lower/src/tags.rs` | Maps those attributes to the new properties. |
| `host/apple/Sources/ExactKit/IOS/NavigationIOS.swift` | Configures UIKit's bar and navigation items, attaches a content scroll view, and forwards a trailing bar action to an authored control. |

The patch requests large or inline titles, a minimal back button, an SF Symbol
trailing button, and a large title that collapses with scrolling. It finds a
content scroll view by walking descendants. It also updates bar visibility as
the selected controller changes. These details show that the problem includes
object discovery and lifecycle timing as well as property access.

James said he had not looked over the patch and was still working on the first
build. Its contents are evidence of the requested behavior, not a tested
implementation or a complete patch review.

*(Claude)* **Exact has prototyped this behavior before.** On 2026-09-10,
[LLP 1035.001 D9](1035.001-native-interaction-ownership.rfc.md) built the same
mechanism: a native title, with the route's scroll view as the bar's content
scroll view. It stopped after three implementation rounds. The bar looked right;
the scroll coordinates did not. With a 106-point header, UIKit's changing top
inset moved a requested `scrollTop` of 80 to 132 and a requested 0 to −52. The
same Contract in the browser reported 0 and 80. Charlie then ruled for D9's
header-shaped route "after a prototype, no `title` prop now".

Separately, [LLP 1037 F1](1037-dartnative-lessons.research.md) records how
DartNative removed a blur flash after a pop: one system bar for the whole stack,
never hidden or shown across a transition. The supplied patch adds a title prop,
uses D9's content-scroll-view mechanism, and hides the bar for untitled routes
at each transition. Nobody has checked whether these hazards appear in James's
app. They would apply to any implementation of this behavior, whatever the
extension mechanism.

### 2.2 Easier patching does not resolve the upgrade concern

James considers making Exact easy to patch, but prefers users not to need it.
His concern is that a new Exact release requires updating all those patches,
without “a clean public API with careful breaking changes.” Agents making a
patch inexpensive to produce does not remove the maintenance obligation.

*(Claude)* The supplied patch shows that obligation in miniature. Its five schema
rows take ids 216–220. The highest id on `origin/main` at `54ae031df` is 215, so
the next five properties added upstream will claim the same ids. From then on, a
release can conflict in `schema.json`, `tags.rs` and `NavigationIOS.swift` at
once.

He says his gut reaction is to use UIKit directly, because he knows he can use
the APIs he needs and difficulties will not arise from Exact's abstraction.
He identifies the costs of that choice too: laying out an app twice is annoying,
and larger apps should share logic.

NativeScript is raised as a possible way to call arbitrary native members
synchronously from Contract or TypeScript. James describes that as an escape
hatch rather than the recommended way to do everything. This is a candidate,
not his acceptance of a particular bridge or a rejection of Swift app code.

### 2.3 Tabs require control over containers and state lifetime

James's subsequent report says tab bars are not well supported and Exact does
not maintain state for each tab. He is working on a patch using
`UITabBarController`, with each tab able to specify its controller type. He is
unsure how that should work on the web. No tab patch was supplied in this
conversation, so its implementation and exact state-loss mechanism are unknown.

There is an important qualification to investigate: the current
[router value](../route/src/lib.rs) contains a retained stack per tab, and
[`select`](../route/src/router.rs) switches those stacks (reselecting the current
tab truncates it to its root). That does not establish retention of native
controllers, Contract-local state, scroll positions, or editor state. James's
observed failure must be reproduced before assigning a cause.

*(Claude)* The router LLP states the designed behavior.
[LLP 1038 §7](1038-router.rfc.md) lists, among what it deliberately does not add,
"Retained *views* for unselected tabs (their entries are retained, their rows are
not)". The trigger it names for adding them is "a measured Interview complaint
about a tab forgetting its place", and it says "It needs one navigation owner per
tab in the Apple projection." Apps render the selected tab's stack
(`each e in stack(nav)`, e.g. `apps/realworld/app.contract:276`). So an
unselected tab losing its scroll position and local state is the expected result
of today's design. James's report is the kind of complaint that trigger names;
a reproduction still has to confirm his case is this one.

Two more facts bear on the tab case:

- **Today's iOS tab bar is a view, not a container.** It is
  [LLP 1059](1059-tab-bar-projection.rfc.md)'s: a symbol-over-label `tablist`
  projects to a `UITabBar` drawn inside the tablist's own box. No
  `UITabBarController` is involved.
- **The web already has the parts.** Its host hides and inerts every route but
  the selected one (LLP 1038 §1 and D6), and a tab switch is a history entry
  (D12).

## 3. What a satisfactory answer must cover

1. **API access.** Properties, methods, object construction, callbacks, protocols,
   and subclassing where the app needs them; a way to reach new SDK and
   third-party capabilities without one Exact release per API.
2. **The right objects.** A developer can reach or supply the actual navigation
   controller, route controller, navigation item, and intended content scroll
   view. Creating an unrelated UIKit object is insufficient.
3. **Ownership and timing.** The app's changes have defined lifetimes and do not
   fight Exact's layout, reconciliation, delegates, or navigation state.
4. **Container composition.** Native containers and custom controllers can host
   shared content, with explicit containment, appearance, layout, and teardown.
5. **State retention.** Switching tabs preserves the state promised by the app;
   route history, mounted UI state, and durable application data are distinct.
6. **A supported extension boundary.** App customizations depend on a documented
   contract whose changes are deliberate and visible, rather than private
   presenter internals. The compatibility commitment itself needs a decision.
7. **Shared authoring where useful.** Native access need not require abandoning
   Exact's common layout and logic. Platform-specific behavior has an explicit
   web disposition, with no implied browser implementation of UIKit.
8. **Exact's operational constraints.** The web remains the CSS semantic oracle;
   optional native machinery has an explicit cost; UIKit thread affinity,
   runtime ownership, reload, and first-pixel behavior remain accounted for.

The phrase “no ceiling” here concerns access to supported public platform APIs
and control over composition. It does not promise that every API becomes
portable, that platform restrictions disappear, or that unrestricted mutation
of a framework-owned object is coherent.

## 4. Existing work and the scope of this discussion

LLPs 1024 and 1067.000 supply native views and app-specific calls. LLP 1031
supplies Exact surfaces inside native apps. LLP 1035.001 governs navigation and
interaction ownership, and LLP 1038 models routing. These are foundations for
an answer, not proof that James already has the extension points he needs.

[LLP 1035.006, item 12.08](1035.006-public-ui-coverage.plan.md) recommends semantic
properties or optional components rather than a general native styling escape
hatch. It is a Draft plan. James's report is a concrete reason to revisit that
recommendation, with the cost of per-property patches now visible.

[The repository's current scope policy](../rules/DEFERRED.md) also declines
public API stability before 1.0 and general JS UI authoring. Writing this
requested discussion does not change those policies. A recommendation that
changes them must name the change, consumer, and scope trade before it becomes
implementation work. No implementation owner or date is assigned by this parent.

## 5. Contributions

| LLP | Author | Purpose |
|---|---|---|
| [1075.000](1075.000-native-platform-control-research.research.md) | Codex | Existing Exact seams, external interop options, evidence and unknowns. |
| [1075.001](1075.001-native-platform-control-recommendation.rfc.md) | Codex | Proposed direction, alternatives, and the proof needed before selection. |
| [1075.002](1075.002-native-platform-control-claude.rfc.md) | Claude (Opus 5.5) | A recommendation. r1 was written without reading 1075.001; r2 revised it after reading, and its §10 records what it took and where it still differs. Claude's research is folded into 1075.000 as F6–F10 and E6–E8. |
| [1075.003](1075.003-native-platform-control-merged.plan.md) | Claude (Fable 5.1) | The merged plan Charlie asked for on 2026-10-01: 1075.001 and 1075.002 r2 as one document. It corrects r2 on five points where 1075.001 was right, puts six decisions to Charlie, and adds three things neither recommendation covered. It is a Draft and accepts nothing; the two recommendations stay as the record of the argument. |

Further independently authored children should use the next unallocated child
number after checking the live corpus and deleted-document history. Numbers
beyond these two are not reserved. Contributions may disagree; adding a child
does not accept it or revise this problem statement to favor it.

## 6. Open questions for the proposals

- Is Swift app code an acceptable normal escape hatch for James, and which
  operations specifically need direct synchronous TypeScript access?
- Which tab state is lost, under what authoring pattern, and in which revision?
- Does James need configuration of Exact-owned containers, replacement of those
  containers, or both in his current app?
- How should portable routing and platform-owned presentation divide authority?
- What compatibility promise can Exact make for extensions before 1.0?
- What behavior and measured costs would distinguish a useful public extension
  from moving the framework's maintenance burden into app code?
- *(Claude)* Are James's two cases extension points or framework features? Both
  change geometry or lifetime that Exact also controls: the content area and
  scroll coordinates under a title (§2.1), and which tab's UI stays alive (§2.3).
- *(Claude)* Does the 2026-09-10 ruling against a title prop stand, now that a
  user has written one?
- *(Claude)* Contract lowers no `data-*` attributes today. Every attribute is a
  row in a fixed table, and only a module tag's leftover attributes travel as
  one aggregate (LLP 1024 §9). Should an app have a vocabulary of its own?

## 7. Working-set trade

The three new documents replace the `llp/current/` links to LLPs 1048.000,
1048.003, and 1067.000, keeping the working set at 15. The rendering parent 1048
continues to point to its two specs; 1067.000 is linked from this family. This
archives overlay links only and changes no document's status or implementation
scope.

*(Claude)* On 2026-10-01 the merged plan, 1075.003, took 1075.001's link, so
the working set stays at 15. 1075.002 was never linked. Both recommendations
remain in the corpus and are linked from the plan and from §5. Restoring
1075.001's link means removing another.
