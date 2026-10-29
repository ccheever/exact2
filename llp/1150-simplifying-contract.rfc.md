# LLP 1150: Simplifying Contract

**Type:** RFC
**Status:** Superseded by [LLP 1035.005.002](1035.005.002-fewer-rules-to-combine.rfc.md) (2026-10-28), kept as the record of one of two blind drafts on the same question (the other is Astra's LLP 1035.005.001). Its four blind reviews (`llp/reviews/rfc-2026-10-28-1150-and-1035.005.001-r1.*`) found it NOT READY: its counts (the operators, Shelf's root) and several citations were wrong, as 1035.005.002 §R records; do not cite its numbers.
**Systems:** Contract (`contract/syntax`, `contract/types`, `contract/lower`, `contract/cli` `migrate`), the plan (`plan/tables/format.json`), the runner, the JS target (`host/web-js`), the Lean semantics and difftest (`semantics/`), the packages (`packages/`, `exact.json`), `docs/contract-grammar.md`, `docs/start-here.md`, `docs/card.md`, `contract vocab`
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-10-28
**Related:** LLP 1017 (the restart: what the record praised, §2.7 governance instead of language), LLP 1017.000 P4c (root-only resources, `type-child-resource`), LLP 1081 (names Exact invents; stage 2 deferred), LLP 1088 (what the diaries asked), LLP 1091 and 1091.002 (`use`, `members`), LLP 1116.016 (expressions are TypeScript's; D0 what Contract never becomes; §6 stage 2, absence as `T | undefined`), LLP 1125 (`sequence`, MIDI), LLP 1126 (Live Activities), LLP 1111 (`tool`), LLP 1045 and 1045.000 (`decoration`, `atom`), LLP 1133.001 (packages: `exact.json`, sources and sends), `rules/RULES.md` ("Delete; don't deprecate")

## 0. Summary

Contract's core is right and small: `state`, `derive`, `action` with inferred writes, keyed `for`, `testId`,
`send` with the reply as a slot, routes as a value, gated tasks, tests in the language. Every author's record
praises those (LLP 1017 §3) and they are what makes the static plan possible. What has grown around them is
not core: a second spelling for absence and for every logical operator, a declaration ceremony on every
resource, and a grammar section for each host capability. The grammar reference is 2,486 lines; the language's
own EBNF is about 150 of them.

This RFC proposes six changes. Each removes something; none adds a construct. They are ordered by what they
remove from a real file, measured on the 169 `.contract` files under `apps/` and `packages/` (§1).

| | Decision | Removes | Migration |
|---|---|---|---|
| D1 | Absence is `T \| undefined`, read with `?.`, `??` and narrowing; `option<T>`, `some`, `none` and the inline `match` go | 583 option lines, two `match` forms | `contract migrate`, mechanical |
| D2 | One spelling per operator: `&&`, `\|\|`, `!` | `and`, `or`, `not` (2,472 lines) | `contract migrate`, mechanical |
| D3 | A host capability is a package, not a grammar section | `sequence`/`play`, Live Activities, MIDI, `tool`, `decoration`, `atom`, `wakelock`, `orientationlock`, the media session, from the parser | per capability, few users |
| D4 | A resource's or mutation's answer is a type annotation | `as shape` (832 lines), `mutation`'s clause order | `contract migrate`, mechanical |
| D5 | A child component may own a resource outside a `for` row | the pressure that made `members` | none; `members` stays until unused |
| D6 | The `-exact-` names an author writes are roles; the rest are audited against CSS | names that cover what CSS already names | per name, with LLP 1081 stage 2 |

D1 and D2 are the ones a builder meets on every screen. D3 is the one that decides whether the grammar keeps
growing. D5 is a question before it is a decision (§5).

## 1. The evidence

Counted over the 169 `.contract` files under `apps/` and `packages/` (`node_modules` excluded) on 2026-10-28.

| Construct | Lines | Note |
|---|---|---|
| `and`, `or`, `not` | 2,472 (146 files) | `&&`, `\|\|`, `!`: 7 lines |
| `as shape` | 832 | on 593 `resource` and 239 `mutation` lines, every one |
| `case some(` | 376 | 207 of them in the inline `match x { case … }` form |
| `pending(` | 302 | the reply slot read before its `match` |
| `option<` | 19 | the annotation is rare; the absence is not |
| `task` | 94 | one timer primitive, used everywhere: keep |
| `sequence`, `tool`, `decoration`/`atom`, `startActivity` | 1, 2, 3, 2 | each has a grammar section |
| `members` | 10 | Shelf's root is 1,137 of its 1,312 lines |
| `-exact-` names | 55 distinct | 392 `-exact-secondary-label`, 175 `-exact-label`, 168 `-exact-footnote` lead |

The unguided-sketch corpus (LLP 1116.016 §1) writes `null` or `undefined` in 29 of 80 features and `??` or
`?.` in 10; guided builders learn `match` and write it in 46 of 80 screens. Neither spelling is wrong; two is.

## 2. The rule this RFC applies

LLP 1116.016 D1: an expression Contract accepts means what the same TypeScript means, apart from a closed
table of deviations; what it refuses is refused with the TypeScript that works. D0 bounds the language: no
statements in expressions, no recursion, no functions as values, no effects on change, bounded work. This RFC
changes nothing in D0. It applies D1 to the forms D1 did not reach (absence, the operators) and extends the
same test to declarations: **a declaration form is syntax only when no package, type annotation or existing
construct can say it.** Where two forms say one thing, the TypeScript one stays and the other is deleted, with
`contract migrate` rewriting what exists (`rules/RULES.md`, "Delete; don't deprecate").

## D1. Absence is `T | undefined`

A value that may be absent has type `T | undefined`. `undefined` is the one absent value; `null` is refused
with the edit. `x?.f` reads `f` when `x` is present and is `undefined` otherwise; `x ?? d` is `x` when present,
else `d`, evaluating `d` only then. A comparison `x != undefined` (or `== undefined`) narrows `x` through
`&&`, `||`, a ternary's arms and a view `if`'s body; `if x` on a non-boolean stays refused, as today. `find`,
`at` and a list index yield `T | undefined`, as `noUncheckedIndexedAccess` has it. A nested
`T | undefined | undefined` is `T | undefined` (today's `bounds.rs` already refuses a nested option).

`option<T>`, `some(x)`, `none`, `case some`/`case none` and the inline `match x { case some(a) => …, case none
=> … }` are deleted. `match` stays only if stage 2's RFC finds a use the three forms above cannot spell; the
expectation is that it goes, and a view's `match` becomes `if x != undefined`.

The snapshot is the hard part, as LLP 1116.016 §6 says: an action reads the state it began with, so narrowing
must stop at an assignment, and the refusal of a read after an assignment must say in one line that the read
sees the old value. That example is the RFC's test. LLP 1116.016 §6 lists the decisions stage 2 must make and
its two gates (the diagnostic, and Charlie and James reading one option-heavy app faster in the new spelling);
this RFC takes them as its own and adds nothing to them. What it adds is the conclusion the gates should reach
if they pass: the old forms are deleted, not kept beside the new ones.

The data boundary emits `T | null` today (`contract/cli/src/typescript.rs`); it changes to `T | undefined`
so a `resource`'s answer and the data module's return type agree.

Before: `let track = match s.tracks.at(0) { case some(t) => t.id, case none => "" }`
After: `let track = s.tracks[0]?.id ?? ""`

Before: `fails when match logged { case some(a) => not a.ok, case none => false }`
After: `fails when logged?.ok == false`

## D2. One spelling per operator

`&&`, `||` and `!` stay; `and`, `or` and `not` are deleted from the lexer's sixteen reserved words
(LLP 1088 D5) and from the grammar. `contract migrate` rewrites the 2,472 lines. The refusal of a stray
`and` names the edit.

The counter-argument is the corpus: the words are what every file in the repo writes, and they read well. The
answer is D1's rule: the words are a second spelling of a TypeScript form, every builder's prior is the
symbols, and a corpus is a migration, not a reason. The migration is one run.

## D3. A host capability is a package

Shelf reads the camera, barcodes, text recognition, NFC, quick actions and the search index through `use …
from "@exact/…"`: each is a package whose `exact.json` declares sources, sends and shapes (LLP 1133.001), read
by a `resource` and sent by a `send`, with keyword arguments the package declares. That is the one mechanism.
Each of these has instead a section of the grammar and a keyword in the parser:

| Today | As a package |
|---|---|
| `sequence name when gate` with `play(tracks, bpm=, clock=, …)` (LLP 1125) | `send playing = play(tracks, bpm=…)` from `@exact/transport`; its gate is a `task`'s; `exactTransport()` stays a resource |
| `startActivity`, `updateActivity`, `endActivity` (LLP 1126) | sends from `@exact/live-activities` |
| `exactMIDI`, `sendMIDI`, `midimessage` | a resource, a send and an event from `@exact/midi` |
| `tool name writes idempotent closed "…" = action fails when …` (LLP 1111) | `app.json` `intents` names the action and its policy; the Contract line goes |
| `decoration`, `atom`, `atoms`, `atomFaces`, `atompress` (LLP 1045) | the Markdown editor's package declares them; a `textarea markup="markdown"` takes them as attributes |
| `wakelock`, `orientationlock` | sends from `@exact/screen` |
| the media session's events and commands | `@exact/media-session` |

What a package cannot say today, and this RFC would add to packages rather than to the grammar: a declared
event heard on the root (LLP 1133.001 D6 has it); a gated send, which is `task … when gate` with the send in
its action (already possible); a send whose value is re-read after every commit, which `sequence` does and
which a package's resource does too (a resource asks again when any of its arguments changes, `docs/contract-grammar.md` "Views"). The
question in §5 is whether that last mapping holds for `play`'s "a changed value ends the voices not yet
started": if a resource's re-ask cannot say it, `sequence` stays and the other rows still move.

The test for any future capability: if it can be a package, it is one; a keyword needs this RFC's §2 rule
answered in its own LLP.

Each row's users are few (§1), so each migration is small; the win is the grammar reference, which becomes
the language plus `contract vocab`, and a parser that stops growing a case per platform feature. LLP 1017 §2.7
is the record of where the other path ends.

## D4. The answer is a type annotation

```
resource library = library() : Library                 // was: … as shape Library
resource page = fetchPage() with q : Page else empty() // was: … with q as shape Page else empty()
mutation setPage : Ack queue refreshes library then arm  // was: mutation setPage as shape Ack queue …
```

`as shape T` is deleted; `: T` is the annotation a `state` and a parameter already use. `mutation`'s clauses
keep their meaning and order; only the ceremony goes. `contract migrate` rewrites all 832 lines.

## D5. A child may own a resource outside a `for` row

Today only the root owns `resource`, `mutation` and `task` (`type-child-resource`). LLP 1017.000 P4c gives
the reason: a row must not open N requests, and only the root has a clock. The first reason is real and
narrower than the rule: it forbids a resource inside a `for`, not inside a child. The second is the runner's
and the plan's: one clock, one request table, and the compiler already inlines every child into the root
(`contract_syntax::expand`), renaming its state and actions per use.

The proposal: the compiler lifts a child's `resource`, `mutation` and `task` into the root as it lifts its
state, one copy per use, and refuses a use inside a `for` body with the reason P4c gives (`type-row-resource`:
"a row must not open a request per item; lift it to the list's owner"). The runner and plan are unchanged:
they see root resources, as today. A child with a resource is then a screen that owns its data, as a React
component does, and the root stops being the place every send must live.

Why it is here: `members` (LLP 1091.002) exists to split a root that holds everything, and Shelf shows the
root holding 87% of the app. If D5 lands, `members` has fewer reasons to exist; it is not deleted by this RFC,
and is deleted later only if no file uses it. §5 asks whether the lifting is as simple as it looks.

## D6. The `-exact-` names an author writes are roles

The role names (`-exact-secondary-label`, `-exact-footnote`, `-exact-fill`, the system colours, the layout
regions) are what an author writes instead of a colour or a size, and they carry the whole "leave it unsaid"
rule: keep them. The rest are properties CSS has no word for and Exact does (`-exact-press-scale`,
`-exact-press-haptic`, `-exact-spring`, `-exact-fit`, `-exact-tint-color`, `-exact-drag-timeline`,
`-exact-layout-transition`, `-exact-exit-animation`, `-exact-continuous`, `-exact-mode`, `-exact-menu`,
`-exact-scroll-edge-effect`, `-exact-hairline`, `-exact-copy-`, `-exact-placeholder`). For each: is there a
CSS property, a role, or an attribute that says it? `-exact-tint-color` on a symbol already lost to `color`
(`QUEUE.md`, 2026-10-23), which is the shape of the answer: where two names cover one thing the CSS one
stays. This is LLP 1081's deferred stage 2 with a list; the audit is its first step and each deletion its own
line in that LLP's table.

## 3. What stays, and why

- **`task`** (94 uses): the one timer primitive, gated, with `every` and `after`. It is how D3's gated sends
  and D5's child timers are written.
- **`send`, `refresh`, `pending()`, the reply as a slot:** the web's stale-while-revalidate (LLP 1016). D1
  makes the reply `T | undefined`, which is what `pending()` and `?.` read.
- **Routes as a value:** `nav = push(nav, "/add")`. Nothing simpler says the same thing on every host.
- **Keyed `for`, `testId`, inferred `writes`, `derive`:** LLP 1017 §3.
- **Tests in the language:** they compile to the agent's operations; the grammar's largest section, and
  earned.
- **D0:** no statements in expressions, no recursion, no functions as values, no effects on change, bounded
  work. The reason Contract is not TypeScript, and the reason it can be checked, baked and proved.

## 4. Migration and order

Each decision is its own lane with its own `contract migrate` rule, fixtures and difftest run, landed in this
order: D2 (one day, no semantics change), D4 (one day), D1 (stage 2's RFC and gates; the semantics change),
D3 one row at a time, D6 one name at a time, D5 after its question in §5. A lane deletes the old form in the
same landing that admits the new one; nothing is deprecated.

## 5. Open questions

1. **D1:** does `match` keep a use that `?.`, `??` and narrowing cannot spell? A view that draws two
   different subtrees is `if x != undefined … else …`; a value is `x ?? d`. If none is found, `match` goes.
2. **D3:** can a package's resource with `with` arguments say what `sequence` says about a changed value
   ("ends the voices that have not started, plays the new value from that commit")? If not, `sequence` stays
   and is the one keyword a package cannot replace.
3. **D5:** the compiler lifts child state per use today. What in the lifting of a resource is not the same
   move: its `with` arguments reading child state (already renamed), its `refreshes` naming a sibling, its
   `else` placeholder? A child's resource in a package (`packages/cards`) would also be the first resource a
   package owns, which LLP 1139 D6's `lower-library-platform` must still refuse for host facts.
4. **D2:** `not` is also a word in `fails when not …` (the `tool` line, which D3 removes) and in tests. Does
   any test form read better with the word, and does it matter?
5. **D6:** which of the fifteen non-role names is a role in disguise (`-exact-hairline` is a width,
   `-exact-mode` a media query)?

## 6. How it is judged

The instrument is LLP 1116.016 §7's: guess-and-repair builds on the real hosts, the number to drive down the
refusals a builder meets that no message repairs in one edit. For D1 and D2 the measure is the one that RFC's
stage 2 sets. For D3 the measure is the grammar reference's length and the parser's case count, which should
fall and not rise again. For D4, D5 and D6 the measure is the line count of a root component in the apps that
exist, Shelf first.
