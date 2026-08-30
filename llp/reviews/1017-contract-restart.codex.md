# Panel: LLP 1017 Contract, started over — §8, the forks (codex)

- **Family:** codex (OpenAI): `codex exec -s read-only -C <capsule> --skip-git-repo-check --ephemeral -m gpt-5.6-sol -c model_reasoning_effort="ultra" -o codex.md`, the panel brief on stdin (the run log confirms `model: gpt-5.6-sol`, `reasoning effort: ultra`); workdir = the panel capsule (31 files: the authoring capsule of LLP 1017.001/1017.002 — seven Coterie diaries, exact2 LLP 1004/1005/1006/1012/1016, the rules, both app contracts, `rejects.txt`, the tag table, exact1's 0508 excerpt, guide, caltrain twin, todo field report, Coterie's chat route, bubble, and one sidecar, the evidence extract — plus LLP 1017 r2, 1017.001, 1017.002, and the original brief), read-only sandbox, no network.
- **Method:** a panel at Charlie's request, 2026-08-30 (LLP 1016 §6's sense: not a refine loop; no verdict binds). One round, mutually visible: each panelist saw LLP 1017 r2 and the other family's sub-LLP; neither saw the other's panel answer. Panel brief sha256 93a84e19c0a4c9e682028e0be06015350731e85eaf28466113716a45dba6a444; LLP 1017 r2 sha256 636e2a108ecf321d5bcce0655cf7c948fb77e4471ce3291dfb0f92fade6f57b3; capsule sha256 bb796924c7da95ccbc045065926e61886b25ee1139d778ecaeee3cdfc3b7d1da. Run 2026-08-30T13:19:51Z → 2026-08-30T13:27:29Z.
- **Disposition:** the fold is recorded in LLP 1017 §10.

---

## Round 1 — position (received 2026-08-30T13:27:29Z, verbatim)

# Panel: LLP 1017 — the forks (GPT-5.6 Sol)

## F1

My position: keep expression-only `fn` in Contract, but narrow the claim.
It covers scalar presentation work; it does not cover every recorded escape.
The first thing it cannot do already exists: traverse and join lists for
`decorateFeed` or `bubbles`. That work stays behind an explicitly declared
Rust data seam, not an undeclared roster export and not a larger `fn`.

The record splits cleanly. Palette selection can be a closed expression
(diary 0086). `decorateFeed` joins feed rows to profiles, while
`bubbles(history, profiles, members, viewerId)` computes quote excerpts and
aggregate receipts (diary 0102); both require list traversal. Computed access
and `scrollToMessage(id)` add one missing expression and one host effect
respectively (diary 0103). LLP 1017 §5 P5 is right to stop before iteration.

Evidence that would change my mind: if a full Coterie port records zero
expression-only helpers after classifying every escape, I would drop `fn`
and take Contract-declared Rust `pure`.

## F3

My position: Grok and 1017 r2 are right; submission belongs under `submit`.
Keep `key` for Escape, arrows, and other editing commands. A command field may
declare both, but Return dispatches `submit`; portable submission logic must
not branch on `k == "Enter"`.

On macOS and iOS, text-field `key` exposes editing commands rather than the
web’s complete `KeyboardEvent.key` stream, so my branch was web-only
(LLP 1017 §5 P2/F3, citing LLP 1008 §5). LLP 1005 §6 records the current
web-name payload, not three-host equivalence. Diary 0098 shows the cost of
mistaking a web input detail for the authoring contract: the compiled handler
missed real typing and required a capture-phase provider bridge.

Evidence that would change my mind: I would remove `submit` only if one
three-host fixture, including IME composition, delivered Return through `key`
with identical committed text and no duplicate dispatch.

## F4

My position: concede `status`; use the value and pending flag independently.
For a resource, render the current value and optionally add
`when pending(resource)`. For a mutation, `match` its `option<T>` for
idle/result and inspect `pending(mutation)` separately.

LLP 1016 D3 defines two simultaneous facts: the resource retains its compiled
or last-settled value, and `pending(resource)` reports revalidation. Exclusive
`status` preserves the old value only by moving it into a `pending previous`
arm, forcing the author to duplicate or factor the ready subtree. `match` plus
`pending()` makes keep-previous the default. Diary 0103’s praise for exhaustive
async regions supports visibility, but not a form that hides stale data.

Evidence that would change my mind: if three real screens repeat inconsistent
`match`/`pending` combinations and a nonexclusive `status` sugar removes them
without duplicating or changing the rendered tree, I would restore that sugar.

## F5

My position: a child owns `state`, `derive`, and `action`. The live-instance
identity rule—slot declaration, component-use address, and enclosing `each`
keys—is right: a keyed reorder retains state; removing the row or leaving the
arm destroys it. A dev reload, however, carries nothing in this cut.

`derive` was one of the repeatedly praised forms, and diary 0102 used it to
turn message history into receipt and quote presentation; child-local
`hotTestId` and receipt marks are the same class. LLP 1005 §6 already proves
that keyed rows survive reorder. But LLP 1004 D5 explicitly makes reload a
full teardown from initial state with “no identity matching.” My draft and
1017 r2 improperly smuggled state-preserving reload into P4.

Evidence that would change my mind: I would add reload carry only after a
prototype survives insert, move, rename, and key changes without stale matches
and still meets LLP 1004 D5’s 100 ms p50 restart budget.

## P7

My position: `expect state name == value` is a `state` read followed by a test
assertion, not a ninth operation. Tests belong in the language but beside the
app—such as `app.test.contract`—and compile only in test mode. Grok’s compiled
`has node testId` is useful as a static check, but not enough as the test block.

LLP 1012 §1 defines `state` as one of eight operations and specifies its typed
reply; `expect` compares that reply. LLP 1012 §5’s current smoke already lives
beside the app and checks actions, state, layout, clocks, and hosts. A compiled
`has node` cannot establish conditional presence, dispatch, settlement, or
layout: diary 0102’s unbounded scroll and diary 0098’s typing failure both
compiled while the behavior failed. LLP 1017 §2.7 records why static, literal
`contract` clauses were insufficient.

Evidence that would change my mind: I would colocate tests in the app if sibling
tests require duplicate declarations or miss rename errors, and accept
compiled-only `has node` if a Coterie rerun finds no failure beyond those checks.

## Inconsistencies

My position: the root-only rule wins, and my Caltrain sample changes.
The application root owns `state searchText` and the three queries
`nearby`, `allStations`, and `matches`; it passes their values plus
`setQuery`, `selectStation`, and `back` into `StationsScreen`.
`searchFocused` and `lastKey` remain child state, while `StationRow` retains
local `hot` and its derive.

My document says only an application root owns `query`, `mutation`, and
`timer`, specifically to prevent row components from opening N requests
(LLP 1017.002 §Reactivity and effects). The sample then declares three queries
inside non-root `StationsScreen` (LLP 1017.002 §Caltrain stations).
LLP 1017 r2 §9 identifies the contradiction correctly; the sample is wrong.

Evidence that would change my mind: I would reconsider child queries if a
Coterie port shows root lifting recreates 20-plus-prop interfaces and the
compiler can statically bound child request identity and cardinality.

## Where 1017 r2 misreads me

- “A child `derive` is a pure alias resolved at inlining, costs nothing at
  runtime” (§5 F5). I said it is an automatically tracked calculation owned
  by an instance; I did not say it disappears at inlining or costs zero.

- “Sol restores an exhaustive `status` form (`idle` / `pending previous` /
  `ready value`)” (§5 F4). I gave that triad to mutations; queries had only
  `pending value` and `ready value` because bake guarantees an initial value.

- Tests “whose statements are the agent API’s own verbs,” including
  `expect state` (§5 P7). I said the eight operations produce replies and
  assertions read them; `expect` is test grammar, not an operation.

## What I concede

- Grok and 1017 r2 are right on `submit`; my `key == "Enter"` is not portable.

- Grok and 1017 r2 are right that `pending()` plus the existing value expresses
  stale-while-revalidate better than exclusive `status` arms.

- LLP 1017 r2 is right that my Caltrain sample violates my root-only data rule.

- Grok is right that list joins and aggregation belong at the data seam;
  expression-only `fn` covers only the smaller half of the escape problem.

- My reload-carry claim contradicts LLP 1004 D5; reload resets until that
  decision is reopened explicitly.

- My top-level test example settled the language but not placement; tests
  should live beside the app.