# RFC 0493: Text-Input Behavior Declarations — keystroke filtering as data, three enforcement tiers

**Type:** RFC
**Status:** Review
**Systems:** Renderer, Apple Host, Windows Host, Android, Web, Contract (compiler/runtime), Rust Native, kernel, protocol, Agent API (Acto), Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-20 (r3 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen
this revision: ONE enforcement-eligibility rule replaces r2's three
stories (§4.5 matrix is the authority; correctness-bearing declarations
fail eligibility without the hook; cosmetic degrades to commit-gate
with effective-tier reporting; JS reconcile never a declared-filter
fallback; every "Tier A degraded" phrase deleted); the Rust hook's
false fuel/deadline/panic-reject guarantees withdrawn (arbitrary native
code cannot be fueled or same-thread-preempted; 0331 embedded release
panics abort — verified) and the tier split into the recommended
bounded predicate artifact (placement-neutral, host-evaluated) vs the
embedded-only App-ABI callback with watchdog semantics; the hook
reclassified off the capability registry (frozen 0485 class-2 forbids
app-authored code in capability impls) — 0499 governs the edge, not
the callback identity; `maxlength` joins the one-gating-truth
constraint set with the pinned composition order; enforcement
arbitration made host-authoritative and node+generation-fenced (worklet
install is async — compile check is an early diagnostic only); the tel
accept-set text fixed (real proposed set incl. `#`/`*`, fixture-pinned)
and the fixture set named a versioned authority freezing observable v1
semantics before protocol rows; `Value` cited as the LLP 0370
terminal-channel sibling.)
2026-08-20 (r2 — program super-refine round 1. Load-bearing
correction: the shipped ENG-23560 same-keystroke **worklet `filter`
prop** falsified r1's "no synchronous gating exists" and "worklets never
gate." r2: declarative prop renamed **`filterClass`**; the worklet
filter recognized as the JS-native Tier B hook and §4.4 restated as a
closed three-mechanism gating rule; the **pre-editor hook** made the
authoritative same-keystroke gate (0236's lifecycle is buffer-first, so
the commit gate audits/reconciles); unsupported correctness-bearing
declarations fail tier eligibility; census gains `Editable`;
`maxlength` unit pinned as an open decision (UTF-16 recommended); v1
accept-sets sketched; the Rust hook gains fuel/deadline/panic/
reentrancy obligations, RFC 0499 governance, and a placement
disposition.)
2026-08-26 (r4 — dual-family review round on r3 (codex `gpt-5.6-sol`
xhigh + `grok-4.6`, mutually blind); **both NOT READY**. Reviews at
`llp/reviews/0493-text-input-behavior-declarations.{codex,grok}.md`. Folded:
the three tree-verified factual corrections only (LLP 0236 is Implemented, not
Draft; the "four input-behavior props" line rephrased as four text-input
value/trait channels; the "strictly more capable than JS-on-native" claim
withdrawn — the shipped ENG-23560 worklet falsifies it). Added §11 recording
the round: both families' first material concern is that this RFC is recorded
by LLP 0507 §6.6 and RFC 0491 Phase 2 as the owner of revision-1 event-payload
decisions and makes none of them, so acceptance would appear to discharge a
rider it does not discharge. Seven material concerns and the pre-acceptance
open questions are recorded UNFOLDED because each is an author decision.)
2026-08-20 (r1 — initial draft)
**Related:** LLP 0236 (presenter-owned text editing — the transaction/session-revision substrate this RFC's commit gate plugs into; its filter slot is split here into the shipped ENG-23560 gate + a future transformation-only slot, §4.4), LLP 0173 / LLP 0170 (presenter text editing program — the IME/dictation validation matrix this RFC's composition rules inherit), LLP 0297 / LLP 0322 (threading contract — why per-keystroke runtime-thread JS is impossible on native while the §4.3-sanctioned worklet filter and the Rust hook are not), RFC 0491 (Kernel Refresh Program — the WS-B typed-prop/enum and WS-C binary-event windows this vocabulary should land inside), RFC 0480 / LLP 0481 / LLP 0485 (flat plan — Contract grammar freeze coordination for the new `input` attrs), LLP 0472 (web-standards-first runtime APIs — the "no second ways" discipline; §9 names this RFC's one deliberate extension beyond HTML), LLP 0091 (`llp/contract/` — Contract forms, validation, and form-scoped errors: the owner of validity/`pattern`-class semantics this RFC explicitly does NOT claim), RFC 0085 (`llp/contract/` — contract-block clause grammar the input-constraint claim extends), RFC 0092 (`llp/contract/` — input/focus semantics), LLP 0331 / LLP 0333 (Rust Native roots / App ABI — the no-JS consumer of the synchronous hook), LLP 0160 §5.2 (Contract-first binding rule), LLP 0370 (W4 controlled-value channel — the `Value` prop's existing semantics), `docs/callback-affinity.md` (the affinity row the Rust hook must declare), ENG-23550 (the controlled-TextInput revert — the failure mode this RFC removes the need for), `tests/protocol/protocol-inventory.json` (the prop/event authority the vocabulary enters through), `issues/20260808-native-textinput-first-keystroke-caret-jump.md`

## Summary

"A phone-number field accepts a digit and rejects a letter" is today
impossible to express in Exact without putting JS on the keystroke path —
the exact pattern that produced React Native's controlled-input jank and
Exact's own ENG-23550 revert — unless the author writes an ENG-23560
worklet filter, which is imperative JS a no-JS root cannot carry. The
wire carries four input-behavior props (`SecureTextEntry` 64, `Editable`
65, `Placeholder`, LLP 0370's controlled `Value`); there is no input
mode, no length cap, and no *declarative* character filter, and
therefore no way for a no-JS Rust Native app to have a phone-number
field at all.

This RFC makes the accept/reject decision **data, not code**: a small,
closed, HTML-mirrored vocabulary declared on the element (`inputmode`,
`filterClass`, `maxlength`), carried as typed props, and enforced
synchronously by the host's own text machinery — the **pre-editor hook
is the authoritative same-keystroke gate**, with LLP 0236's transaction
commit gate as the auditing and reconciling backstop for every mutation
path the hook cannot see. Because the decision is data,
Contract, React, and no-JS Rust Native express it identically and get
identical behavior; because it is host-enforced, there is zero JS and zero
flicker on the keystroke path. Two escape tiers sit beside the declarative
road: a genuinely synchronous imperative hook where synchrony is
structurally free (web `beforeinput`; a main-affine in-process Rust hook —
a real Rust-tier differentiator), and an honestly-labeled asynchronous
reconcile tier for Hermes JS on native, made safe by 0236's revision model.

The kernel is the plumbing, never the decider: the vocabulary enters the
protocol inventory as typed props and closed enums (inside RFC 0491's WS-B
window, before the generated tables freeze), events ride WS-C's binary
frames, the declaration projects into semantics (the right keyboard comes
up; agents see the constraint), and the filter becomes **claimable** — a
contract block can state that an input accepts only digits, and Acto
verifies it by typing a letter and asserting rejection.

## 1. Motivation

**The current state, verified against the tree (2026-08-20):**

- `tests/protocol/protocol-inventory.json` carries four text-input-specific
  value/trait channels — not an exhaustive list of props that affect input
  behavior, since input-applicable `Disabled`, `SelectionStart`,
  `SelectionEnd`, and `TextContent` also exist — `SecureTextEntry` (64), `Editable` (65, the LLP 0297 A3
  read-only channel), `Placeholder`, and `Value` (87 — the LLP 0370 W4
  *terminal-editing* controlled channel, a sibling prop, not a
  general phone-field substrate: kernel TextInput semantics still read
  from `text`) — and **no input mode, no declarative filter, no max
  length, no keyboard-type hint of any kind**.
- *Declarative* filtering therefore requires a JS round-trip: keystroke →
  change event → runtime thread → JS validation → controlled-value
  write-back → host. LLP 0297 makes that round-trip necessarily
  asynchronous on native, so the rejected character *appears and then
  vanishes* — caret disturbance and flicker as an architectural
  guarantee, not a bug. ENG-23550 (the controlled-TextInput revert) and
  `issues/20260808-native-textinput-first-keystroke-caret-jump.md` are this
  mechanism's existing case law.
- One synchronous mechanism **does** already exist and this RFC builds
  beside it rather than pretending otherwise: the ENG-23560 **worklet
  `filter` prop** (LLP 0297 §4.3) — a `'worklet'`-directive predicate
  whose accept/reject/replace verdict runs on the resident UI worklet
  runtime inside the native preflight, before anything renders
  (`builtin-schema.ts`, `host-ops.ts`; exercised by
  `js/src/text-inputs-lab`). It is imperative, JS-authored,
  per-keystroke code — the arbitrary-predicate tier — and it degrades
  to the async-revert contract on hosts without the ops. What it is
  not: declarative, agent-legible data; usable from a no-JS Rust root;
  or a semantics hint that raises the right keyboard.
- LLP 0236 (Implemented) built the correct substrate — presenter-owned editing
  sessions, `TextEditingTransaction`s with `baseRevision`, the host bridge
  accepting or rejecting each transaction, stale controlled echoes rejected
  by revision — but ships no vocabulary for *what* to reject. Its filter
  story is a deferred "optional worklet pre-commit filters" slot.
- A Rust Native root (LLP 0331) has no path to any of this: no props to
  declare, no hook to implement, nothing.
- Agents cannot see input constraints: nothing in the semantics tree says
  "this field is numeric," so nothing brings up the right keyboard, and no
  claim can be written about rejection behavior.

**Why this is urgent now rather than generically desirable:** RFC 0491's
WS-B is about to generate the typed prop/enum tables and WS-C the binary
event frames from their declaration authorities, and LLP 0481/0485 are
freezing Contract grammar. Adding the vocabulary now costs a few rows in
authorities that are being regenerated anyway; adding it after costs a
migration through frozen tables. The no-JS direction makes the gap
load-bearing: a Rust-tier app with a phone-number field is about the
smallest realistic form any app has.

## 2. Constraints

1. **The keystroke decision has a hard deadline and a fixed venue.** It
   must happen before the character renders, inside the platform's text
   machinery (UIKit `shouldChangeCharactersIn`, AppKit
   `shouldChangeTextIn`, DOM cancelable `beforeinput`, the Win32 edit
   path) — on main, where IME composition, autocorrect, paste, and
   dictation already live. The kernel is on the runtime thread and is never
   on this path (LLP 0297); its role is carrying the declaration in and the
   accepted value out.
2. **IME safety is non-negotiable.** A filter must never reject
   mid-composition; filtering evaluates at composition commit. Paste,
   dictation, and autofill arrive as transactions, not keystrokes, and are
   filtered as transactions. The LLP 0173 §6d validation matrix (CJK,
   dictation, hardware/software keyboards) is the acceptance surface.
3. **Web standards first, one small named deviation** (LLP 0472). The
   vocabulary mirrors HTML wherever HTML has the primitive (`inputmode`
   verbatim, `maxlength` with HTML's insertion-filtering semantics). HTML
   has **no declarative keystroke filter** — the web implements filtering
   via `beforeinput` — so this RFC adds one closed enum (`filterClass`, §3) as
   its single deliberate extension, and says so rather than pretending
   otherwise (§9).
4. **One gating truth per element.** For a given element, accept/reject
   semantics derive from one declared source: the **Tier A declarative
   constraint set** — `filterClass` *and* `maxlength` together, one
   canonical set with a pinned composition order (character filter →
   per-class strip/reject → length truncation → selection update) — or
   one imperative Tier B hook — the shipped ENG-23560 worklet filter on
   JS-authored native surfaces, `beforeinput` on web, the Rust hook on
   Rust roots. Declaring `filterClass` and installing a worklet filter
   on the same element is a conflict. **Enforcement is host-authoritative
   and runtime-fenced**: props, HMR, and direct frames are dynamic and
   the worklet install rides an async host call, so the host arbitrates
   per node **and generation** — atomic switching between sources, a
   deterministic refusal (`input_filter_conflict`) when both arrive for
   the same node generation — and the compile-time check is an early
   diagnostic, never the sole enforcement. The pre-editor hook and
   0236's commit gate both *derive* from the element's one source. No
   parallel filter vocabularies — the LLP 0150 authority discipline
   applies.
5. **Validity is not filtering, and it is owned elsewhere.** Form-scoped
   validation, error association, and `pattern`-class validity semantics
   belong to LLP 0091 (`llp/contract/`). This RFC gates characters; 0091
   judges values. The boundary is stated in both directions in §8.
6. **Graceful degradation, one rule (the §4.5 eligibility matrix is the
   authority).** A **correctness-bearing** declaration (one a contract
   claim or app behavior depends on) **fails tier eligibility** on a host
   without the pre-editor hook for its class — dormant-but-diagnosable,
   the effective tier reported in semantics; a claim over it cannot pass
   there. A **cosmetic** constraint may degrade to commit-gate
   enforcement, with the effective tier reported. **JS reconcile is never
   a fallback for a declared filter** — Tier C is where undeclared app
   logic lives, not where declarations land. Never a crash, never a
   silent claim of enforcement not performed.

## 3. The declarative vocabulary (v1)

Three attributes, entering `tests/protocol/protocol-inventory.json` as
typed props with closed enums (WS-B window):

- **`inputmode`** — HTML's enum, verbatim: `none | text | decimal |
  numeric | tel | search | email | url`. A keyboard/semantics hint, never a
  filter (exactly as on the web). Projects into semantics so the right
  keyboard appears on iOS/Android and agents see the field's nature.
- **`filterClass`** — the keystroke gate; a closed accept-class enum, v1 set:
  `none` (default) | `digits` | `decimal` | `tel`. Enforced at the
  pre-editor hook and at the 0236 commit gate (§4.1). **Naming decision
  (r2):** the r1 spelling `filter` collided with the shipped ENG-23560
  worklet `filter` prop (§1) — reusing the name would have silently
  broken existing apps; `filterClass` is the recommended spelling
  (final spelling reviewed with the OQ6 grammar pass). Deliberately
  **not** named `accept` (HTML's `accept` is the file-type attribute —
  LLP 0475 territory) and deliberately **not a regex**: per-keystroke
  regex is an IME hazard (composition produces intermediate strings that
  legitimate input must pass through), a cross-platform divergence
  machine (four regex engines), and a ReDoS surface. Classes are
  implemented once per host against a conformance fixture set; v1
  accept-set sketches so OQ1 is refinement, not invention — `digits`:
  ASCII `0-9` in v1; `decimal`: `digits` plus at most one decimal
  separator (locale handling is OQ2's fixture decision); `tel`
  (proposed): `digits` plus `+`, `-`, `(`, `)`, space, `#`, and `*`
  (the DTMF pair carried by real dialers), exact membership pinned by
  the fixture set before any host ships. **The fixture set is a
  versioned authority**: per-class transaction fixtures (keystroke,
  IME-commit, paste, dictation, agent-write) land with the WS-B
  protocol rows, and observable v1 semantics — including per-class
  paste strip-vs-reject — are frozen there before protocol freeze,
  never left to per-host judgment.
  Growing the set is a registry addition, not a design event (OQ1).
- **`maxlength`** — HTML's *insertion-filtering* behavior (including
  paste truncation per platform convention). **Counting unit — open
  decision (r2), with a recommendation:** HTML counts UTF-16 code
  units, and Exact's editing offsets are already explicitly UTF-16
  (LLP 0236); the recommended unit is therefore **UTF-16 code units**,
  with grapheme-cluster edge cases pinned by fixtures as *documented
  outcomes*, not a second counter. If the author instead chooses
  user-perceived (grapheme) units, that is the vocabulary's **second
  named deviation from HTML** and §9 must list it beside
  `filterClass`. One unit ships; hosts never implement two.

Explicitly absent from v1: `pattern` (validity, → LLP 0091), masks and
format-as-you-type (transformation, not gating — §4.4 and OQ3), and any
open-ended predicate syntax.

## 4. Enforcement: three tiers and a narrowing

### 4.1 Tier A — declarative, host-enforced (the paved road)

The declared props drive two coordinated points, both deriving from the
same data:

- **The pre-editor hook** (authoritative for the same-keystroke
  guarantee): UIKit/AppKit change delegates, DOM cancelable
  `beforeinput`, the Windows host's edit path reject the non-conforming
  insertion before the platform editor applies it. This point *must* be
  the gate, because LLP 0236's own keystroke lifecycle updates the
  platform editor's local buffer synchronously **before** the host
  bridge accepts or rejects the transaction — a commit-gate-only
  enforcement is post-render by construction and cannot be flicker-free
  (r1 had this inverted). Composition-safe per constraint 2: mid-IME
  text flows untouched; the filter evaluates the composition's commit
  string.
- **The LLP 0236 commit gate** (audit and reconcile): the host bridge's
  existing accept-or-reject step for `TextEditingTransaction`s applies
  the same declared filter to every mutation path the pre-editor hook
  cannot see — paste, dictation, autofill, agent-driven
  `exact_set_value` — and *audits* the hook (a non-conforming
  transaction reaching the gate on a hook-bearing host is a diagnosable
  host defect, reconciled through the revision model). Paste handling
  follows platform convention per class: `digits`/`tel`/`decimal` strip
  non-conforming characters rather than rejecting the whole paste (the
  iOS phone-field behavior); the strip-vs-reject default per class is
  OQ2's fixture decision.

The two points cannot drift because neither owns semantics: both project
the same declaration. Enforcement honesty follows constraint 6's one
rule, stated as the matrix in §4.5. This is the answer for the
phone-number case on every
tier — a Contract app, a React app, and a no-JS Rust app declare
`inputmode=tel filterClass=tel` (spelled per surface) and get identical,
synchronous, flicker-free behavior with zero code where the hook exists,
and an honest eligibility failure where it does not yet.

### 4.2 Tier B — synchronous imperative hooks (where synchrony is free)

- **Web JS**: `beforeinput` is the platform — main-thread, cancelable,
  standard. Exact adds nothing and takes nothing away (LLP 0472).
- **JS-authored native surfaces: the shipped ENG-23560 worklet filter.**
  The existing `filter` worklet prop *is* this tier on native for JS
  apps — a `'worklet'`-directive predicate serving
  accept/reject/replace verdicts from the resident UI worklet runtime
  inside the native preflight (LLP 0297 §4.3's sanctioned shape: the
  worklet runtime is main-owned, so no cross-thread bounce). r1's claim
  that Hermes-authored apps have no synchronous tier was wrong;
  what they lack is a synchronous tier running *ordinary app JS on the
  runtime thread* — that remains physics (LLP 0297 §4.4), and worklet
  eligibility restrictions apply.
- **Rust Native — two shapes, honestly priced (r3).** The r2 text
  promised fuel/deadline enforcement and panic-safety an arbitrary
  in-process native callback cannot deliver: arbitrary native code is
  not instruction-fueled, a same-thread deadline cannot preempt it, and
  LLP 0331's embedded release builds compile `panic = "abort"` — a
  panic terminates the application process. Those guarantees are
  withdrawn. The two honest shapes:
  - **(a) The bounded predicate artifact (recommended paved Tier B):**
    a restricted, host-evaluated predicate — a declarative superset of
    `filterClass` (character-class table / DFA-shaped, wasm-hostable
    later) — that the *host* evaluates inside the pre-editor hook.
    Bounded by construction, placement-neutral (embedded and external),
    and classifiable as data through the App-ABI.
  - **(b) The arbitrary in-process callback (embedded-only, honest
    guarantees):** `fn(&EditProposal) -> EditDecision`, main-affine,
    one declared `docs/callback-affinity.md` row; reentrancy forbidden
    (a hook may not drive the editor); a wall-clock *watchdog verdict*
    (an over-deadline hook is treated as reject-with-diagnostic and the
    element's hook is disabled for the session — never a
    nondeterministic accept); and the 0331 panic posture stated
    plainly — a panic in embedded release aborts the process, exactly
    as any app-code panic does. External placement cannot run it
    (0331 forbids synchronous UI-thread waits on app code).
  **Classification (r3):** frozen 0485's class-2 capability rule forbids
  capability implementations that execute app-authored code, so shape
  (b) is **not a capability** — it is an **App-ABI callback**
  classification (0331/0333's surface), while shape (a)'s artifact is
  ordinary declared data. The capability stack governs *whether a
  surface may install an input gate at all* (RFC 0499's edge), not the
  callback's identity. Both shapes' decisions are recorded through the
  0236 transaction stream so the session revision model stays the
  single history. **Open decision:** ship (a) alone in v1, or (a)+(b);
  recommended default: (a) alone — (b) only with embedded-only labeling
  and the watchdog semantics above.

### 4.3 Tier C — asynchronous reconcile (honest, and steered away from)

JS validation on native is after-the-fact: error states, format-on-blur,
mask application on commit, cross-field logic. Per-keystroke *gating* from
this tier carries reconcile semantics — the value may momentarily show and
correct — and LLP 0236's revision model is what makes even that honest
(the correction is a new revision; stale echoes cannot clobber the live
editor). Documentation and lint steer per-keystroke gating to Tier A; the
`filterClass` classes are designed to cover the real cases so Tier C gating
stays rare.

### 4.4 The gating-mechanism ruling (restated in r2)

r1 ruled "worklets may transform but never gate" — falsified by the
tree: the ENG-23560 worklet filter ships and gates today (§1). The
ruling that survives, and what it forbids:

- **Gating mechanisms are closed at three**: the Tier A declarative
  constraint set (`filterClass` + `maxlength`, one set with the pinned
  composition order — constraint 4), and the two Tier B hooks (the
  ENG-23560 worklet filter for JS apps; the Rust predicate
  artifact/callback for Rust roots — web's `beforeinput` is the
  platform's own instance of the same slot). Per element, exactly one
  gating source, host-arbitrated per node generation (constraint 4).
- **No fourth mechanism, ever**: LLP 0236's still-deferred mask/
  formatting slot is *transformation only* (OQ3) and may not grow a
  second accept/reject vocabulary; nor may Tier C JS reconcile be
  dressed up as a gate.
- The declarative road is the paved one: docs and lint steer new
  gating to `filterClass` first, the worklet/Rust hooks second, and the
  conformance fixtures exercise all three against one IME story.

LLP 0236 takes the matching amendment at this RFC's acceptance (§8):
its filter slot is not "deferred" — it is split into the shipped gating
hook (governed here) and the future transformation slot (OQ3).

### 4.5 The enforcement-eligibility matrix (the one rule)

Constraint 6's rule, as the authority every other sentence derives from
(execution tier vs reported tier vs claim eligibility):

| Host state for the declared class | Correctness-bearing declaration | Cosmetic declaration |
| --- | --- | --- |
| Pre-editor hook implemented | Tier A enforced; semantics reports Tier A; claims eligible | same |
| Hook absent, commit gate present | **Fails tier eligibility** — dormant-but-diagnosable; semantics reports the effective tier (commit-gate); claims over it cannot pass | Degrades to commit-gate enforcement; semantics reports the effective tier |
| Neither | Fails tier eligibility; semantics reports unenforced | Dormant, reported unenforced |

JS reconcile (Tier C) never appears in this matrix: it is where
*undeclared* app logic lives, not a fallback for declared filters. A
commit-gate-only host is never called "Tier A" — the gate is post-render
by 0236's own lifecycle and cannot be flicker-free.

## 5. Kernel and protocol contribution (plumbing, not deciding)

- **Props**: `inputmode`, `filterClass`, `maxlength` as typed props with closed
  enums in the inventory authority — rows in WS-B's generated tables, not
  stringly one-offs.
- **Events**: change/composition/rejection events ride WS-C's binary event
  frames. Whether a distinct `inputrejected` event ships for UX feedback
  (shake, haptic) or rejection stays observable only via the transaction
  stream is OQ4.
- **Semantics**: the declaration projects into the semantics tree — the
  correct virtual keyboard on touch platforms, and an agent-visible
  constraint (`exact_tree` shows the field's mode and filter; agents stop
  guessing).
- **Claims**: the contract-block grammar (RFC 0085, `llp/contract/`) gains
  an input-constraint clause so a component can state the behavior and
  have it machine-checked three ways — statically (the attr exists and is
  a known class), in tests (conformance fixtures), and live (Acto types a
  rejected character via the real typing path and asserts the value did
  not change; physical-platform claims obey the real-input evidence rule).
  Clause spelling is OQ5, decided in 0085's grammar, not here.

## 6. Authoring surfaces

- **Contract**: `input inputmode="tel" filterClass="tel" maxlength=12 ...` —
  new attrs on the `input` tag, coordinated with the LLP 0481/0485 grammar
  freeze (OQ6 tracks the spelling review; `filterClass` sits beside the
  existing worklet `filter` attr, which keeps its shipped meaning).
  Contract binding ships first per LLP 0160 §5.2.
- **React tier**: the same props on the input component, within one
  checkpoint of the Contract binding.
- **Rust Native**: the same three typed props on the input builder, plus
  the Tier B hook — the first input capability where the Rust tier is
  the first input capability available to a **no-JS** root at all. (r4: this
  read "strictly *more* capable than JS-on-native" until both reviewers
  falsified it — the shipped ENG-23560 worklet already supplies arbitrary
  accept/reject/replace, and shape (a) is a DFA, which is weaker. The real
  differentiator is no-JS/embedded reach.) Worth stating in the tier's
  docs.
- **Facet**: `FacetInput`-family components pass the vocabulary through;
  no Facet-level re-invention.

## 7. Sequencing

1. **Now (freeze windows):** inventory rows + enum authorities (pre-WS-B
   generation), Contract grammar addition (with 0481/0485), the
   conformance fixture set (filter classes × IME/paste/dictation cases ×
   platforms — the LLP 0173 §6d matrix extended).
2. **Apple first**, riding LLP 0236's implementation slice (its stated
   target): pre-editor hook + commit gate from declared data.
3. **Web**: the props map near-verbatim to platform behavior (`inputmode`,
   `maxlength` are native; `filterClass` is a small `beforeinput` shim in the
   host, not app code).
4. **Rust hook** with LLP 0331/0333 governance and the affinity row.
5. **Android/Windows** as their host programs reach text editing; until
   then the §4.5 matrix's hook-absent row governs — cosmetic constraints
   degrade to commit-gate enforcement with the effective tier reported;
   correctness-bearing declarations fail tier eligibility there
   (constraint 6; never called Tier A).

## 8. Amendments owed at acceptance

- **LLP 0236**: its deferred filter slot split per §4.4 — the shipped
  ENG-23560 worklet gate governed here; the future mask/formatting slot
  transformation-only; the **pre-editor hook** named the authoritative
  same-keystroke gate and the commit gate named the audit/reconcile
  point for declared filters (0236's buffer-first lifecycle makes any
  other assignment post-render).
- **LLP 0091** (`llp/contract/`): a boundary paragraph in both documents —
  0493 gates characters at input time; 0091 owns value validity, error
  association, and anything `pattern`-shaped. A field can carry both.
- **RFC 0085** (`llp/contract/`): the input-constraint claim clause (OQ5).
- **`docs/callback-affinity.md`**: the Tier B Rust hook row.

## 9. Honest costs and tensions

- **`filterClass` is a deviation from web standards** — HTML has no declarative
  keystroke filter. The alternative ("just use `beforeinput` everywhere")
  fails on native, where JS is not on the keystroke path and cannot be
  (LLP 0297); the alternative alternative (regex-per-keystroke) fails on
  IME, divergence, and ReDoS grounds (§3). A closed enum enforced by hosts
  is the smallest extension that makes the common cases data. The LLP 0472
  posture is satisfied by naming the deviation, mirroring HTML everywhere
  else, and keeping the web mapping a thin shim over the standard event.
- **Two enforcement points per platform** (authoritative hook + audit
  gate) could drift; they don't get to, because both are derived projections of
  one declaration and the conformance fixtures exercise both paths per
  class.
- **A closed class set will be asked to grow** (currency, alphanumeric-id,
  hex…). The registry-addition path (OQ1) is deliberately cheap; the
  pressure to add an open predicate form is deliberately resisted — that
  pressure is Tier B's job.
- **Grapheme/locale edge cases** in `maxlength` and `decimal` (locale
  decimal separators) are real; the fixture set, not per-host judgment,
  pins them (OQ2). If the `maxlength` unit decision (§3) lands on
  user-perceived units instead of the recommended UTF-16, that is a
  second named HTML deviation and belongs in this list's first bullet.

## 10. Open questions

- **OQ1 — The v1 class set and its growth path.** Is `digits | decimal |
  tel` the right v1? Registry mechanics for additions (authority row +
  fixture set + per-host table), and whether a class may be
  platform-gated.
- **OQ2 — Paste strip-vs-reject defaults per class**, and the grapheme/
  locale fixture decisions (locale decimal separator handling in
  `decimal`).
- **OQ3 — Masks and format-as-you-type.** The transformation slot's
  future: a declarative mask vocabulary (worklet- or host-implemented), or
  Tier C formatting forever? Coordinates with LLP 0091's display-format
  concerns; explicitly deferred from v1.
- **OQ4 — Rejection feedback.** A first-class `inputrejected` event
  (haptic/shake UX) vs. observation via the transaction stream only.
- **OQ5 — The claim clause spelling** in RFC 0085's grammar (e.g.
  `state input testId="phone" filters digits` vs. a dedicated clause
  head), decided with 0085.
- **OQ6 — Contract attr spelling review** with the 0481/0485 grammar
  freeze (`inputmode` lowercase-HTML vs. camelCase house style — the
  0472 tension in miniature).
- **OQ7 — `exact_set_value` and agent writes.** Agent-path writes go
  through the commit gate (§4.1) — should an agent be able to bypass the
  filter with an explicit flag for test-setup purposes, or is a filtered
  field only ever fillable with conforming values (default: no bypass;
  conforming values only)?

## 11. The r3 dual-family review round, and what it left to the author

*(Added r4, 2026-08-26, by LLP 0506 D1(a) lane L-D1A. r3 folded round-2
materials and was never itself reviewed; this section records the round that
reviewed it. Two mutually blind reviews — codex `gpt-5.6-sol` at xhigh and
`grok-4.6` at high, neither seeing the other's text — are at
`llp/reviews/0493-text-input-behavior-declarations.codex.md` and
`llp/reviews/0493-text-input-behavior-declarations.grok.md`. **Both returned
NOT READY.** The lane folded only the factual corrections both families
verified against the tree; every item below is a decision this RFC's author
owns and an agent may not take.)*

### 11.1 The finding both families reached independently

**This RFC is recorded as the owner of EXWF revision 1's event-payload
decisions, and it makes none of them.**

LLP 0507 §6.6 puts on revision 1's closed wire-visible input list *"RFC 0493's
typed input props and event-payload decisions (freeze-window riders)"*, and RFC
0491's Phase-2 row repeats it. This RFC's entire event contribution is one
bullet in §5 — *"change/composition/rejection events ride WS-C's binary event
frames"* — followed immediately by OQ4 asking whether `inputrejected` exists at
all. There is no field list, no discriminant, no bounds, no family id, and no
binary layout. `Composition`, `InputRejected`, and `inputrejected` appear zero
times in `tests/protocol/protocol-inventory.json`, `Change` (event 4) still
carries `payloadFamilies: []` and `payloadSchemaStatus: "activation-gap"`, and
23 of the 24 EXWF event rows remain activation gaps.

The consequence is asymmetric, which is why it is the first material concern in
both reviews: **accepting this RFC as written would appear downstream to
discharge a Phase-2 rider that it does not discharge.** The program plan
already reads it that way.

Two ways out; the author picks one, and whichever it is must land **with** the
acceptance rather than after it:

- **(a) Own them.** Add a normative table mapping every event concept this RFC
  introduces to an inventory event id and a concrete EXWF payload-family
  schema — at minimum `Change`, plus a closed yes/no on a rejection family and
  a composition family. This does not make this RFC the owner of the other
  ~20 families.
- **(b) Disclaim them.** Amend LLP 0507 §6.6, RFC 0491's Phase-2 row, and the
  schema package's closed-wire-visible-inputs projection so that this RFC's
  rider covers its three typed props and their enums only. The 23 families then
  need a named owner, which they do not currently have.

Either way, LLP 0507 §8.3 binds: if payload schemas are added after revision 1
activates, they require a revision and digest bump — another break.

### 11.2 The other material concerns, unfolded because they are decisions

| # | Concern | Raised by | Why the lane did not fold it |
| --- | --- | --- | --- |
| 1 | §4.5's `correctness-bearing` vs `cosmetic` split is not machine-decidable from the declaration. "A contract claim exists" is detectable; "app behavior depends on it" is not a prop, not an enum, and not an algorithm a host can evaluate. Both families independently traced a case where a declaration silently receives the weaker tier and nothing tells the author. grok additionally reports the hook-absent cell as internally contradictory: "dormant-but-diagnosable" and "semantics reports the effective tier (commit-gate)" cannot both hold | both | Picking the rule (default everything to correctness-bearing with an explicit `best-effort` opt-in, versus keeping the split and putting intent on the wire) changes the wire vocabulary |
| 2 | §4.2(b)'s watchdog cannot deliver a mid-keystroke verdict. The hook is main-affine inside `shouldChangeCharactersIn`; a same-thread watchdog cannot fire until the callback returns, and an off-thread one cannot make the delegate return `false` while the hook still runs. A hung hook wedges the UI with no in-process recovery | both | Whether v1 ships the bounded artifact only, or artifact-plus-callback, is a scope decision. Both families recommend artifact-only for v1 |
| 3 | `maxlength`'s unit is still an "open decision" on a freeze-window prop. UTF-16 vs grapheme changes what `12` means for an emoji ZWJ sequence or `e`+combining acute, and naive truncation at a code-unit boundary can produce a lone surrogate or silently drop an accent. codex adds that the integer's own wire type is unspecified (range, zero, absent/clear, overflow refusal) and warns a plain TS `number` would derive a float | both | Pinning the unit is a semantic decision with a permanent wire consequence |
| 4 | §2 constraint 4's node/generation-fenced arbitration is asserted, not specified. The shipped install/remove operations carry only `{nodeId, source}` and the filter map is keyed by bare `UInt32`; the existing generation handling fences worklet-runtime ownership, not node-allocation reuse | codex | Requires naming the fence coordinate and the Tier A/B transition protocol |
| 5 | §8's amendment list is incomplete. RFC 0499 still describes this RFC's synchronous hook as a capability-shaped edge and an App-ABI capability-handle instance; LLP 0297 A3's replacement table still routes "max length / charset masks" to worklet filters except on secure fields. Accepting r3's reclassification without those amendments leaves two governing documents describing a different object | grok | Amending other accepted documents is the author's call |
| 6 | Secure fields are unmentioned. Under LLP 0297 A3 secure transactions are digest-only, so §4.1's commit-gate audit cannot inspect the text it is asked to audit — while `filterClass` can still run at the pre-editor hook, because the platform editor does see characters | grok | Needs an explicit stated exception |
| 7 | `inputmode` is a hint. Hosts and user agents keep keyboard choice; §3/§4.1's "the right keyboard comes up" is stronger than the platform guarantees. codex recommends reporting the requested mode and whether it was applied, and keeping keyboard appearance out of any correctness claim | codex | Touches what the RFC promises |

### 11.3 Open-question triage, where the families agreed

codex classified every OQ against the freeze window. **Before acceptance:**
OQ1 (v1 enum membership — it is a closed WS-B authority), OQ2 (paste, locale,
grapheme outcomes — observable enforcement semantics meant to be
fixture-frozen with the protocol rows), OQ4 (a rejection event is an inventory
row and a payload family; it cannot cross the revision-1 freeze undecided),
plus the two unnumbered ones: `maxlength`'s unit/type, and whether Rust v1
includes callback shape (b). **After acceptance:** OQ3 (masks are explicitly
out of v1), OQ5 (claim spelling is RFC 0085 grammar), OQ6 (authoring spelling,
though the canonical prop identity must be fixed before the 0481/0485 freeze),
and OQ7 only if "no bypass" becomes the normative v1 rule.

### 11.4 What r4 changed

Only what both families verified against the tree and what takes no decision:
LLP 0236's status (`Implemented`, not `Draft`); the "four input-behavior props"
phrasing, which was read as exhaustive when input-applicable `Disabled`,
`SelectionStart`, `SelectionEnd`, and `TextContent` also exist; and §6's
"strictly more capable than JS-on-native", which the shipped ENG-23560 worklet
falsifies. Everything in §11.1 and §11.2 is untouched and waiting on the
author.

## Revision history

- **r4 (2026-08-26)** — the first review round r3 ever received: two mutually
  blind reviews from different families, both NOT READY. Three tree-verified
  factual corrections folded; §11 added recording the seven material concerns
  and the open-question triage as author decisions. No design decision taken.
- **r1 (2026-08-20)** — initial draft, from the phone-number probe
  discussion. Scope verified against the tree first: `SecureTextEntry`/
  `Placeholder`/`Value` confirmed as the only input-behavior props in the
  inventory; LLP 0236's transaction substrate identified as the
  authoritative commit gate (and its worklet slot narrowed rather than
  duplicated); LLP 0091 (`llp/contract/`) identified as the validity owner
  and the `pattern` boundary drawn accordingly.
