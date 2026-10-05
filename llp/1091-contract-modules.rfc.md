# LLP 1091: Contract modules

**Type:** RFC
**Status:** Accepted by Charlie (r1, 2026-10-04: "Approve recs" on §7). r2 resolves round 1 (Astra max, Grok 4.7 xhigh: both SOUND WITH CHANGES; §9). Stage 1 landed (7cdf080e2), stage 2 (f0a074f32); r3 records the code review (§10)
**Systems:** Contract loader (`contract/cli/src/{sources.rs,symbols.rs,map.rs,rust.rs,lean.rs}`), syntax (`contract/syntax`), the TS bake's capture (`js/bake`), the web build's capture (`host/web-js/build.mjs`), the dev loop and `build.rs` rebuild tracking, `exact new` (`game/new.mjs`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Implementer:** Claude (Opus 5.5) lanes for Charlie Cheever: stage 1 on 2026-10-05, stage 2 on 2026-10-06 (§5)
**Amends:** LLP 1017 P8 (`use`); LLP 1055.002 D1 (a timeline's name and the merged namespace)
**Unblocks:** LLP 1055.002 D7 (module-scoped keyframes) and D8 (`use Activity from "exact:motion"`); QUEUE "Two `.contract` files cannot be shared between apps" (`apps/markdown` and `apps/llp` copy `Blocks`/`Runs`)
**Related:** LLP 1086 (apps outside the repo; "no npm packages yet"); LLP 1030 (delivery: the plan's sha256 is its identity). Research only: LLP 0480 (exact1's `use` imported TS and was closed), LLP 0520 (content-addressed class names with collision detection).

## Summary

Two problems, which are separable:

1. **Scope.** `use X from "./f.contract"` checks that `f` declares `X`, then
   merges *every* declaration of `f`, and of everything `f` used, into one
   namespace (`sources.rs` `Exports::merge`). Keyframes are global outright.
   Two files that each declare `pulse`, `Button`, `Item` or a timeline
   `Pending` cannot be loaded into one app, and a name nobody imported is
   visible anyway (`diagnostics.rs` calls `Row()` through two files).
2. **Reach.** A `use` path must start `./`, have no `..`, and resolve under
   the root file's directory (`contract-use-path`). Three more mechanisms
   assume the same thing: the TS bake compiles `app.contract` inside a staged
   copy of the app, `Sources::relocate` refuses a source outside that stage,
   and the dev loop and Rust apps' `build.rs` watch `app.contract` only. So a
   library can't live outside the app, and there's no `exact:` module for a
   built-in.

The finding that makes this small: **component, style and fn names never
reach the plan.** They are inlined away; `source_locations.rs` asserts that a
multi-file app's plan is byte-identical to the same app in one file. The
names in plan bytes are shape names (`types.name`), font families
(`families.name`) and keyframes names (`keyframes.name`), plus the
`clock(Name)` string a timeline lowers to. So scope is a loader pass:
resolve each file's references against that file's own scope, give each
declaration a program-unique name, and hand the rest of the compiler the
same flat `File` it gets today. Only names that are also *runtime text*
(keyframes, timelines) need more than that.

| | Decision | Stage |
|---|---|---|
| D1 | A file sees its own declarations and what it names in a `use`, nothing transitive | 1 |
| D2 | `use A, B as C from "…"`: a list, and `as` to rename | 1 |
| D3 | Every top-level declaration is importable; no `export` keyword (yet) | 1 |
| D4 | The loader renames, and only on collision; existing plans stay byte-identical | 1 |
| D5 | Keyframes and timelines are scoped too; their runtime text is rewritten where it is written | 1 |
| D6 | Fonts stay app-global, like `@font-face` | 1 |
| D7 | Identity is the declaration, not its text; the "identical declarations merge" rule goes | 1 |
| D8 | `exact:<name>` built-in modules, shipped in the compiler | 2 |
| D9 | A bare specifier is a package, resolved through `node_modules` | 2 |
| D10 | The compiler reports every source it read; capture, relocation, watching and deploy use that list | 2 |
| D11 | Libraries are Contract only: no resources, data code or native modules | — |

Qualified access (`use * as ui`, `ui.Button(…)`) is §6, not taken now.

## 1. Evidence

- **Collisions refused today.** `contract-use-duplicate` for a component,
  shape, style or fn declared differently in two files;
  `lower-keyframes-duplicate` for any two `keyframes pulse`;
  `contract-use-duplicate` for any two `timeline` of one name, even identical
  (LLP 1055.002 D1, ruling 4).
- **Charlie, 2026-10-04 (LLP 1055.002 rulings):** should Contract take module
  scope now, "since `use` cannot reach a library outside the app directory
  and every imported name lands in one namespace".
- **Copies across apps.** `apps/markdown` and `apps/llp` carry the same
  `Blocks`/`Runs` renderer (QUEUE). Lexy declares its own `timeline Activity`
  because there's no `exact:motion` (LLP 1055.002 D8).
- **Who uses `use` today.** In this repo: `apps/messages`, `apps/messages-legacy`,
  `apps/expose`, `examples/{ios,macos}/calendar`, `contract/corpus/use`.
  Outside it: `signal-exact2`. All of them already name each import on its
  own line (`apps/messages/app.contract` has eleven), so D1 costs them only
  the names they reach transitively.

## 2. Decisions

### D1 — A file sees its own declarations and what it names

A file's scope is its own top-level declarations plus each name its `use`
lines bring. Nothing comes along transitively: if `app` uses `Card` from
`ui`, and `ui`'s `Card` uses `Icon` from `icons`, then `Card` still works,
because `Card` is resolved in `ui`'s scope. But `app` cannot write `Icon()`
unless it uses `Icon` itself.

A name declared in the file and also brought by a `use` is refused
(`contract-use-shadows`), as is one name brought by two `use` lines from
different declarations (`contract-use-duplicate`, reworded). This is ES
modules' rule.

The root is still the first component of the root file. `routes`, `test`,
and resources/mutations/tasks stay root-only.

### D2 — `use A, B as C from "…"`

```
use Button, Card as UiCard from "./ui.contract"
use pulse from "./motion.contract"
```

One line may name several declarations; `as` renames one in this file. `as`
is already reserved (`syntax/src/parser/names.rs`). The one-name form is the
one-element list, so every existing `use` parses unchanged. A renamed
component keeps its declared name in diagnostics, with the local name
alongside it (`UiCard (Card in ui.contract)`).

Any kind of declaration is importable by name: component, shape, style, fn,
keyframes, timeline. Keyframes are new; today they come along implicitly.

### D3 — No `export` keyword

Every top-level declaration can be named by a `use`. Under D1 that is safe:
a helper nobody names is invisible, and two libraries' helpers can't
collide. An `export` keyword buys an API boundary for published packages.
That matters after 1.0 (DEFERRED: no public API stability before then), and
it can be added later without breaking any source that already names only
what it uses. Charlie ruled: none (§7 Q1).

### D4 — Rename on collision only

The loader gives every declaration a program-unique internal name:

- its declared name, if no earlier file declares one in the same
  namespace. Shapes and `fn`s share one namespace, as both are called
  `Name(…)` and `types` refuses a shape and a fn of one name
  (`type-fn-shape-name`); components, styles, keyframes and timelines
  each have their own;
- otherwise `{name}__{stem}`, where `stem` is the declaring file's stem
  sanitized to `[A-Za-z0-9_]`, with `_2`, `_3`… appended until unique.
  Example: `pulse__ui`, `Item__orders`. The root file always keeps its names;
  between other files, the first in load order (depth-first, `use` order)
  keeps its name, so the choice is deterministic.

Each file's references (component tags, `class=`, shape types and
constructors, fn calls, `animation`, `animation-timeline`) are rewritten
through that file's scope before the files are merged into the `File` the
rest of the compiler takes. `types`, `analyze`, `lower`, Lean, and every
executor see the same flat namespace they see today, so they don't change.

Why collision-only: an app with no name in two files keeps byte-identical
plan bytes, and so keeps its plan's sha256 (LLP 1030). That is the only
guarantee. An app that relied on D7's removed rule (two files declaring
one shape alike, merged into one) now has two declarations, refused if one
file names both, and a plan with two types if both are lowered. The mangled form is a valid
CSS ident, keyframes name (`motion/src/animation/parse.rs`), timeline ident
(`kernel/src/timeline.rs`) and Lean string. It is a Rust identifier when
the declared name is; a hyphenated shape name was already not one, and
`contract rust` printing it raw is an existing defect this RFC doesn't
widen (QUEUE). The cost: a name in plan bytes depends on what else is
loaded. That's only visible for keyframes built at runtime (D5).

Tools show the program-unique name, which is the declared name except on
a collision (`Card__ui`); that is enough to tell two apart and costs no
second identity. Because every name is unique after D4, `contract
symbols` no longer lets a later `Button` silently replace an earlier one,
the source map's `component_costs` no longer adds two `Card`s together,
and `contract rust` no longer drops a second `Item`. Showing the declared
name and its file instead is a display change for later, if collisions
turn out to be common.

### D5 — Keyframes and timelines: rewrite where written

Their names are runtime text, so a renamed `pulse` has to be renamed in
the strings that name it.

- **Literal text is resolved lexically.** An `animation`/`animation-name`
  string literal, or the literal parts of a template, is parsed where it's
  written and each keyframes name is rewritten through that file's scope.
  `animation=\`pulse ${d}ms\`` in `ui.contract` becomes `pulse__ui …` if
  `pulse` was renamed. `animation-timeline=Name` and the literal
  `clock(Name)` are rewritten the same way (`syntax/src/clock.rs` already
  rewrites the bare-identifier form).
- **The attributes:** `animation`, `animation-name`, `exit-animation` (and
  their camel spellings), on elements and in `style` and native-control
  rows alike: all of them resolve against the one keyframes table
  (`lower/src/svg.rs` `check_animation_names`).
- **A literal written elsewhere is not followed.** A caller's
  `Spinner(fx="pulse 1s")` is a string where it's written; the inliner then
  puts it in the spinner's `animation`. It is not rewritten in either file.
  When `pulse` was not renamed it works as today. When it was, lowering
  reads the substituted literal and refuses it (`lower-animation-name`),
  at compile time, so the case is loud, not wrong. A component that lets
  its caller choose takes a choice (`kind: string` and a `match`) rather
  than a keyframes name.
- **A computed name is not resolved.** A name built at run time is matched
  against the table, as today, unchecked (`animation=\`nope ${n}ms\``
  compiles). No warning is added: the reviews showed a "first word"
  test misses `1s ${name}` and comma lists, and a correct one is the
  motion grammar's, at run time.
- **Shadowing stays.** `animation-timeline=Name` resolves per file through
  the clock pass, which still lets a binding of the name shadow it
  (`clock.rs`); the pass now takes the file's scope instead of the merged
  file's timelines.

This is CSS Modules' rule (local names in the stylesheet they're written
in; anything built in script is on its own) and LLP 1055.002 D7's proposal,
without a runner change: the plan's keyframes table already holds whatever
name the compiler gives it.

### D6 — Fonts stay global

A `font` declares an app-level face from an asset file, and `font-family`
is matched case-insensitively against families and system names, like
`@font-face`. Two files declaring the same family alike are one; declared
differently, they are refused as today (`lower-font-duplicate`). Canvas
surfaces also name fonts by runtime string (`exact.fontAliases`), which a
rename would break.

A library does not ship font files in stage 2: lowering resolves a face
only under the app's `assets/` (`lower/src/fonts.rs`). A library names a
family and its README says which files the app provides.

### D7 — Identity is the declaration

Two files' `fn clamp` are two fns, whether or not their text matches; a
diamond (two paths to one file) is one declaration, by (canonical path,
index). `same_declaration` and "identical declarations merge" go, which also
removes the one place timelines were special-cased (LLP 1055.002 D1). D1's
ruling, "two timelines need two names", holds within a file's scope, and
across files it's handled by renaming.

Shapes are nominal (`Ty::Record(name)`), so two libraries' `Item`s become two
types. That's the right answer, and the first time it's been expressible.

### D8 — `exact:` built-in modules

`use Activity from "exact:motion"` resolves to a `.contract` file compiled
into the compiler (`contract/lib/motion.contract`, `include_str!`), loaded
like any other file under the source path `exact:motion`. The first:

- `exact:motion` — `timeline Activity` (LLP 1055.002 D8). As built it
  carries no keyframes: the corpus's spinners don't agree on one `spin`,
  and no consumer asked for a shared one.
- Nothing else until a consumer asks. Every built-in is a source file an
  author can read, not compiler magic.

Built-ins version with the compiler; there's nothing to install.

### D9 — Packages through `node_modules`

A specifier that isn't `./…` or `exact:…` is a package:
`use Button from "@acme/ui"` or `"@acme/ui/button.contract"`.

- Resolution follows Node: walk up from the using file to
  `node_modules/@acme/ui/package.json`. `exports` maps `.` and `./sub` to a
  string, or to an object's `contract` or else `default` condition; as in
  Node, a package with `exports` offers only what it lists. Without
  `exports`, the bare name is `index.contract` and a subpath the file at
  that path. A bare `card.contract` is a mistaken relative path, refused
  with `contract-use-path`, not looked up as a package.
- A package's own relative `use`s must stay inside the package's canonical
  root, by the same rule that keeps an app's inside the app today. Symlinks
  are followed first, so `bun link` and `"file:../ui"` work.
- Local sharing *is* this. A sibling directory with a `package.json` becomes
  a dependency through `"@me/ui": "file:../ui"` or a Bun workspace, with no
  second mechanism. Publishing to npm is `npm publish` of that directory.
- Versioning is npm's. Two versions of one package in one app are two sets
  of files, so they're two sets of declarations (D7) and don't collide.
- A relative `use` gets one relaxation: `..` is allowed as long as the
  result stays inside the same root (app or package). `contract-use-path`
  still refuses leaving it.

The app needs a `package.json` for this, which `exact new` doesn't write
today (LLP 1086: "no npm packages yet"). It writes one with no dependencies
from stage 2 on.

### D10 — One source list, from the compiler

The loader already knows every file it read. `compile_path_output` returns
it (path, kind: app | package | builtin, package name and version), and its
consumers use it instead of their own walks:

The list is a resolution graph, not only paths (Astra 3): each source's
logical specifier and resolved path, the package root and the
`package.json` it consulted, and the edges between them. It is reported
on failure as well as success, so a watcher still sees a library that
broke the build (Astra 5).

- **TS bake capture.** The bake compiles a staged copy
  (`js/bake/src/lib.rs`), so it stages each package's consulted
  `package.json` and `.contract` files, byte copies (symlinks are
  refused there), under a `node_modules` mirror in the stage. Rolldown's
  and `tsc`'s refusals of anything outside the stage are unchanged.
  `relocate` maps each staged path back through the graph rather than
  one prefix strip; an `exact:` source has no path to relocate and is
  skipped.
- **Web build.** It compiles the original `app.contract`
  (`host/web-js/build.mjs`) and stages only TS/JSON, so it needs nothing
  but the list for its watch.
- **Watching.** The JS dev loop already rebuilds on any app file and drops
  `node_modules` (`host/web-js/dev.mjs`); it adds each listed package
  root. The wasm session (`host/web/src/dev.rs`) and the Rust apps'
  `build.rs` (`rerun-if-changed=../app.contract`) watch every listed
  path.
- **Deploy.** Deploy excludes `node_modules` and freezes every byte the
  bake reads (`scripts/deploy.mjs`), so the snapshot captures each
  package's consulted files themselves (registry, `file:`, workspace and
  linked alike) and the bake reads those, not the live tree. The
  package's name, version and content hash are part of the snapshot's
  identity. A library change that changes the plan ships as any plan
  change does.

### D11 — Libraries are Contract only

`provide`/`inject` names stay one program-wide channel, matched by name
across every component (`inline.rs`): two libraries that both `inject
theme` read the same nearest `provide theme`. That is the context model
(React's is keyed by object, Contract's by name), and libraries name their
injects accordingly (`acmeTheme`). Not renamed by this RFC.

A library can declare components, shapes, styles, fns, keyframes, and
timelines. It cannot own resources, mutations, or tasks (root-only already),
carry TypeScript or Rust, or declare native modules (`app.json`'s roster).
Data reaches a library component through props, `inject`, and slots. Library
data code is a separate design with its own consumer.

## 3. Effect on each implementation

- **Syntax.** `UseDecl` becomes `{ names: Vec<(name, alias)>, path, span }`.
  The parser accepts the list and `as`.
- **Loader (`sources.rs`).** Per-file scope tables, the rename map (D4), the
  rewrite pass, specifier resolution (D8, D9), and the source list (D10).
  `Exports::merge`, `merge_declarations`, `same_declaration` and the
  timeline special case are deleted. This is most of the code.
- **`syntax/src/clock.rs`.** Timeline rewriting moves into the per-file pass.
- **types / analyze / lower / plan / runner / kernel / hosts.** No change.
- **Lean / difftest.** No change: `lean.rs` emits the merged file's names,
  which are unique.
- **`symbols.rs`.** A `use` name navigates to the declaration it means, by
  namespace; `map.rs` and `rust.rs` need no change (D4).
- **js/bake, host/web-js/build.mjs, host/web/dev.mjs, `apps/*/web/build.rs`,
  scripts/deploy.mjs, game/new.mjs.** Stage 2 (D10, D9).
- **The inliner's constructor set.** `record_constructors` (`inline.rs`)
  is the merged file's shapes, so a shape named like another component's
  action prop still stops that prop's substitution. That is today's
  behaviour with one file too; D4 doesn't change it.
- **Apps.** Add `use` lines for transitively reached names in
  `apps/messages`, `apps/messages-legacy`, `apps/expose`,
  `examples/*/calendar`; signal-exact2 is told. Lexy moves to
  `exact:motion`'s `Activity`. `apps/markdown`/`apps/llp` share `Blocks` from
  a package (the QUEUE item), as stage 2's consumer.
- **Docs.** `contract-for-agents.md`, `contract-for-humans.md`, and
  `contract-grammar.md` get the `use` forms and the scope rule.

## 4. Tests

Stage 1, in `contract/cli/tests/it/use.rs` and `source_locations.rs`:
- two files each declaring `Button`, `pulse`, `Item`, `clamp`, and a
  timeline `Pending` load together, and each file's uses hit its own;
- a transitively reachable name is refused with `contract-unknown-*` and a
  hint naming the file that declares it;
- `as` renames, list form, and `contract-use-shadows`;
- a diamond is one declaration;
- every corpus and app plan is byte-identical before and after (D4);
- a renamed keyframes in a literal and a template plays (the runner), and
  shows in the web build's CSS as `pulse__ui`;
- `contract symbols` jumps to the right `Button` of two;
- difftest quick.

Stage 2:
- `exact:motion`'s `Activity` shared by two components in two files;
- a package in a temp `node_modules` (scoped name, `exports`, subpath,
  symlinked via `file:`), and a package's `use` leaving its root refused;
- the bake and the web build compile an app whose library lives outside it;
- touching a library file rebuilds in the dev loop.

## 5. Implementation plan

1. **Stage 1, scope (2026-10-05).** One commit: syntax, loader, symbols, map,
   rust, diagnostics, app migrations, docs. No executor or plan change.
2. **Stage 2, reach (built 2026-10-04).** As built:
   `contract/cli/src/resolve.rs` (specifiers), `contract/lib/motion.contract`,
   `contract::source_graph` and `contract sources` (the graph, on failure
   too), `contract::rerun_if_changed` in every app `build.rs`, the wasm dev
   session's watch of every used file, the JS dev loop's watch of each
   package root (`.gen/dev-sources.json`, written by `exact-web-js
   --dev-reload`), the TS bake staging packages under `node_modules/<name>`
   (two copies of one name refused) and relocating through each, deploy
   capturing the repository of a package linked from outside the app and
   Exact (a registry package is pinned by the captured `bun.lock`), `exact
   new` writing `package.json`, and `@exact/reading` (`packages/reading`, a
   Bun workspace) shared by `apps/markdown` and `apps/llp`: the block model,
   `Runs` and `SchemeButton`. Their `Blocks` had diverged in layout and stay
   each app's. Lexy is not in this repository; its move to `exact:motion` is
   its owner's. Planned as:
   **Stage 2, reach (2026-10-06).** `exact:` built-ins with `exact:motion`;
   package resolution; the source list and its consumers; `exact new` writes
   `package.json`; `Blocks` shared between markdown and llp as the consumer.

## 6. Considered and not taken

- **Qualified access (`use * as ui from "@acme/ui"`, `ui.Button(…)`,
  `class=ui.Card`, `x: ui.Item`).** Renaming solves collisions. Qualification
  costs four grammar positions: `Ui.Button(` fails at `.` in tag position
  today, `ns.fn(x)` is refused as a method call, and `class=` takes only
  bare names. It can be added later without breaking anything here. Charlie
  ruled: later (§7 Q3).
- **Always qualify internal names** (`ui/Button`). Cleaner, but it changes
  every multi-file app's plan bytes and compat id, and puts characters into
  CSS idents and Rust names that would need escaping again. Collision-only
  renaming gives the same guarantees.
- **An `app.json` `libraries` map** instead of `node_modules`. It works
  locally but invents a second package system, and an npm publish would then
  need both.
- **Keyframes stay global** (CSS's own rule). Two libraries' `pulse` would
  still collide, which is the problem this RFC exists for.
- **Runtime name qualification for computed keyframes** (LLP 1055.002 D7 as
  written: a file-qualified id in the table and a runner lookup in the
  caller's file). It needs the runner to know which file a string came from,
  for a case that's unchecked today.

## 7. Open questions for Charlie

1. **`export`?** D3 recommends none for now: everything top-level is
   importable, and nothing is visible unless named. The alternative is
   private-unless-`export`ed from day one, which costs every existing
   library file an `export` per declaration.
2. **`node_modules` and a `package.json` in every app (D9)?** This is what
   makes npm publishing free. It means generated apps get a `package.json`,
   which LLP 1086 deliberately left out.
3. **Qualified access now or later?** §6 recommends later.
4. **Process.** Grok and Astra review this before stage 1, or stage 1 lands
   on this draft (it changes no executor, and every existing plan stays
   byte-identical) and review covers stage 2?

**Rulings (Charlie, 2026-10-04): "Approve recs".** Q1: no `export` keyword
(D3). Q2: packages through `node_modules`, and `exact new` writes a
`package.json` (D9). Q3: qualified access later (§6). Q4: Grok and Astra
review this RFC while stage 1 is built; their findings are folded in before
stage 1 lands, and stage 2 waits on them.

## 8. Revisions

- r0 (2026-10-04): draft.
- r1 (2026-10-04): Charlie's rulings on §7.
- r2 (2026-10-04): the round-1 reviews (`llp/reviews/rfc-2026-10-04-1091.{astra,grok}.md`,
  both SOUND WITH CHANGES), disposed below; stage 1 built against this text.
- r3 (2026-10-04): stage 2 as built (§5) and the code review (§10).
- r4 (2026-10-05): the code review's second round (§11).
- r5 (2026-10-05): the code review's third round (§12).
- r6 (2026-10-05): the code review's fourth round (§13).
- r7 (2026-10-05): the code review's fifth round (§14).
- r8 (2026-10-05): round 6; the bake compiles in place (§15).
- r9 (2026-10-05): round 7 (§16).
- r10 (2026-10-05): round 8 (§17).

## 9. Review dispositions (round 1)

| Finding | Disposition |
|---|---|
| Astra 1 / Grok 4: D4's collision domain misses the shared `Name(…)` namespace and the inliner's constructor set | Taken for the namespace: shapes and fns are one namespace in D4, as built. The constructor set's shadowing of an action prop is today's single-file behaviour; recorded in §3, not changed |
| Astra 2 / Grok 2: D5 misses `exit-animation`; caller literals through props; the first-word warning is wrong; keep clock shadowing | Taken: D5 lists every attribute, narrows the caller-literal promise (a compile-time refusal, not a rewrite), drops the warning, and keeps the clock pass's shadowing per file |
| Astra 3 / Grok 5: D10 needs a resolution graph incl. `package.json`, and the bake and web build differ | Taken into D10 |
| Astra 4 / Grok 5: deploy needs the package bytes, not hashes | Taken into D10 |
| Astra 5 / Grok 5: watcher scope (wasm session, `build.rs`; JS loop drops `node_modules`); expose sources on failure | Taken into D10 |
| Astra 6 / Grok 1: byte identity is conditional on no D7 split; "compat id" is the wrong term | Taken: D4 narrows the claim and says the plan's sha256. Measured: all 122 plans in the repo (apps, examples, corpus, conformance) are byte-identical before and after stage 1 |
| Astra 7 / Grok 3: a hyphenated name is not a Rust identifier | Pre-existing in `contract rust`; recorded in D4, not widened |
| Astra 8: library fonts have no delivery | Taken: D6 says the app supplies a library's faces in stage 2 |
| Grok 6: `provide`/`inject` stay one string channel | Taken: said in D11 |

## 10. Code review dispositions (stages 1 and 2)

Astra and Grok reviewed the landed code (`llp/reviews/code-2026-10-04-1091.{astra,grok}.md`), both
UNSOUND. Every finding is taken, each with a case in `contract/cli/tests/it/scope_review.rs`, and
every plan in the repository stays byte-identical to stage 2's.

| Finding | Fix |
|---|---|
| Astra 4 / Grok 1: every word of an `animation` literal was read as a keyframes name | The rewrite reads the shorthand as CSS does: per comma-separated animation, the first word that is not an animation keyword, a number or a function; in `animation-name`, each item. A computed part glued to a unit is a time |
| Astra 3: a `clock(Name)` literal inside an inline `match` was not rewritten | `match` arms are rewritten as ternary arms are |
| Astra 6 / Grok 2: a local action's curried call, a primitive type, or a roster call was resolved as another file's top-level name | The rewrite tracks bindings (members, parameters, `each`/`match`/arrow/`let` binders); primitives and roster calls are never refused; a used file's shape named like a roster function is renamed |
| Astra 2 / Grok 3: the package was the nearest `package.json` above the file, not the one whose `exports` were read | The package is the directory of the consulted manifest's real path; an exported file that leads out of it is refused |
| Astra 5: one library installed under two names kept only the first in the graph, so the bake staged one | `SourceGraph::packages` lists every name a package was reached by; the bake stages each |
| Astra 8: a manifest whose `exports` refused was not watched | `SourceGraph::consulted` lists every `package.json` read; `build.rs`, the wasm session and the JS loop watch them |
| Astra 7: a generated name (`Helper__ui`) bypassed `use` | Generated names are refused like declared ones |
| Grok 4: alike fonts on different lines did not merge | Fonts compare by family and faces, not position |
| Grok 5: an `exports` condition that is not a path hid `default` | Conditions fall through |
| Astra 1: deploy reinstalled an absolute `file:` or a `link:` from the live tree | Deploy refuses them (a relative `file:` moves with the snapshot) and, after the materialized install, refuses any Contract source outside the captured tree |

## 11. Windows deploy path qualification (2026-10-04)

**Implementer:** Codex, 2026-10-04. This is the Windows completion of D10 and
the final row of §10, within Charlie's Windows/papercut authorization. It does
not change Contract syntax, snapshots, signing, native JS, runtime grants or
the publisher's sequence/receipt rules.

### Evidence and bounded change

At `b89d6b6f0`, after fetching current `origin/main`, the inherited `f708c99cf`
deploy changes have three POSIX-only path predicates. An ignored probe in the
isolated Windows worktree retained `target/deploy-paths/before.{json,log}`:
real files under a directory containing spaces and `#`, their native canonical
paths, Bun 1.4.2, the source hash and each original predicate/result. All three
expected admissions/refusals fail. This is a predicate reproduction, not a
claim that a complete malicious deployment succeeded.

1. The materialized closure drops native drive paths with `startsWith('/')`.
   Check every physical app/package source and every consulted manifest with
   Node's native `isAbsolute`, then the existing canonical-path containment
   check. Skip only the graph's `builtin` entries, which name `exact:` modules;
   a nonabsolute physical source is a refusal, not silently omitted. Drive,
   UNC and extended Windows paths therefore reach the same containment test
   as POSIX paths. The check remains after install and before returning the
   materialized app; all consulted manifests stay in its scope. Canonicalize
   the captured root and file with `realpathSync.native`, then additionally
   require `resolve(root, relative(root, file)) === file`. The native Windows
   relative function folds case, so containment alone would admit a distinct
   case-sensitive `STAGE` sibling beside `stage`. The reconstruction check is
   local to this closure and does not change other `inside()` callers.
2. `/^file:\//` misses `file:C:/...` and `file:C:\...`. Extract the target
   after the existing lowercase `file:` protocol, and use the native path
   parser's nonempty root to refuse rooted paths. On Windows that also
   refuses drive-relative `C:library`, whose per-drive current directory is
   not captured; a leading separator, drive root, UNC or extended root is
   not a relocatable dependency. POSIX root handling stays unchanged.
   All `link:` targets remain refused. Relative `file:../library` stays
   admitted; do not URL-decode, case-fold or reinterpret relative path bytes.
   This is a refusal predicate, never a new dependency resolver.
3. Registry-package classification applies a slash regex to `relative()`.
   Split that native result into native path components and test the existing
   exact `node_modules` component. Scoped/nested registry packages remain
   install outputs; a sibling named `node_modules-extra` remains a possible
   local repository input. Keep canonical package roots and existing repo
   ownership decisions. No arbitrary separator replacement or case folding.

The three small predicates/closure routine will be callable from the focused
test and used by the production call sites, so tests exercise those decisions
rather than a second implementation. Existing general `canonicalPath`/`inside`
callers are retained; concurrent hostile filesystem replacement and a general
cross-host path-policy redesign are not claimed by this change.

### File scope and qualification

- `scripts/deploy.mjs`: the three production call sites and their small shared
  decision helpers; keep the source below 1,500 lines.
- `scripts/deploy-paths.test.mjs`: a focused test file. The existing
  `deploy.test.mjs` executes expensive fixtures at import time, so its name
  filter does not provide an isolated path test. This adds no blocking gate,
  test runner or configuration.
- This amendment and `llp/reviews/1091-windows-deploy-paths.gpt.md` record the
  independent plan/code review and actual evidence.

Use real temporary Windows files/directories with spaces, `#` and Unicode,
including real canonical/extended paths and a junction to an outside sibling.
The production closure must accept in-capture app/package sources and
manifests, skip the virtual builtin, and refuse escaped sources/manifests,
malformed physical relative paths, and prefix siblings. Compare relative
and absolute dependency decisions, and real `node_modules`/scoped/nested
versus similarly named external directories. Add explicit native UNC/drive
path cases as syntax-only where no share or second drive is available; do
not describe them as actual remote filesystem I/O. Preserve the old failing
probe and record these limitations. A real owned case-sensitive NTFS fixture,
when the host permits enabling that property, must distinguish `stage` from
`STAGE`; ordinary case-insensitive aliases must still resolve normally. Report
any unavailable filesystem case capability explicitly.

Run the focused tests and existing applicable deploy tests; run required SDK
checks with a private target and bounded CPU concurrency, reporting unrelated
Windows failures rather than weakening them. Full publish/network/GUI work is
not needed for these local capture decisions. Fetch/review any upstream delta
before committing or pushing; the frozen Skirmish release and other agents'
worktrees remain untouched.

**Qualified on Windows x64, Bun 1.4.2, Rust 1.97.0, base `39018dfa4`:** eight
focused tests / 42 assertions pass without skips, including the actual NTFS
case-sensitive and junction cases. Full private build, 2,326 default tests
(54 existing ignores), strict all-target Clippy, caps and boot pass. The raw
`cargo fmt --all -- --check` command exceeds Windows' argument limit (OS 206);
all 379 targets emitted by `cargo fmt --all -v -- --check` pass unchanged
`rustfmt --edition <target edition> --check` in 25 bounded batches, using
rustfmt 1.9.0-stable. These are cold functional checks, not speed comparisons.
The existing `deploy.test.mjs` stops during its import-time fixture at the
intentional `durable stream-head publication is not qualified on Windows`
refusal (zero completed cases); this patch does not weaken that restriction
or claim the full publisher suite passed. Logs, original predicate failures
and the exact partitioned-format inventory remain in the private worktree's
ignored `target/deploy-paths/`. Both independent source reviews found no
remaining blocker; neither substitutes for the executed tests.

**Combined-head qualification after the final fetch:** upstream `b79156175`
adds typed inline expansion for Lean embedding. Its normal `expand_all` path
still disables those ascriptions. Rebased this four-file correction without
overlap, then reran the full private build, all 2,326 default tests (54 existing
ignores), strict all-target Clippy, caps, boot and the eight/42 focused path
checks: all pass. Repeated the exact Cargo-selected formatting inventory;
all 379 targets pass in 25 batches, while the unchanged single invocation
still exceeds Windows' argument limit. This evidence is separate from the
earlier `39018dfa4` run, not an inference from it.

The actual qualified `39018dfa4` compiler and the rebased compiler both compile
the frozen repair Skirmish `app.contract` (SHA-256
`cd8ce8d4812a6a9ad5a6b7b3f0a0b222cb47ce197a5a32b92e2d9adc91715e61`)
to byte-identical 20,033-byte plans, SHA-256
`fef0b68e26e6756a36c2ff67980d6f2cdca4d6b89901b987f55b096810b3844a`.
Their `contract sources` JSON for the same actual absolute Windows path with
spaces is also byte-identical. The ignored `plan-identity.json` retains both
compiler hashes, commands, graph bytes and outcomes; no frozen source, build
or consumer artifact was modified. This establishes the normal game plan and
source-graph comparison, not general Lean semantic equivalence.
The advisory `cargo run -p contract-difftest -- quick` exits successfully with
"nothing changed" for this deploy-only commit; it therefore adds no claim of
executed Lean corpus coverage.

## 11. Code review round 2 dispositions

A delta review of the fixes (`llp/reviews/code-2026-10-05-1091-r2.{astra,grok}.md`), both UNSOUND,
both read at `a51d6ecab`. Every finding is taken, with cases in `scope_review.rs`; every plan in the
repository stays byte-identical. Separately, `bun scripts/smoke.mjs deploy` passed on caltrain
(the whole delivery path through the snapshot, the materialized install and its closure check).

| Finding | Fix |
|---|---|
| Grok 1 / Astra 1: a local binding hid an imported `fn` of its name, and an unimported library `fn` reached a binding's calls | A call resolves as the type checker reads it: this file's `fn` or shape, then a binding, then the roster. Another file's `fn` or shape named like one of a file's bindings is renamed, so the type checker cannot reach it past the binding; a handler `press=pick(1)` with a bound `pick` keeps it |
| Grok 2 / Astra 4: a computed token claimed the name, hiding a literal one; parentheses reset between template parts | A computed token never claims the name; parentheses and quotes carry across parts |
| Grok 3: keywords matched case-insensitively | Case-sensitive, as the motion grammar reads them |
| Astra 3: a keyword whose slot is full (`linear 1s linear`) is the name; a quoted name | The shorthand's keyword slots are tracked per animation; a quoted word is a name |
| Astra 5: a backtick `clock(Name)` template was not rewritten | A template with no interpolation is a literal |
| Astra 7: `pending`, `failed`, `t`, `path` were refused as another file's names | Compiler intrinsics are never refused; another file's `fn path` is renamed when a file calls the router's `path` without it |
| Astra 6: the bake dropped an `exports` entry that is a link inside the package | The bake stages every `.contract` file of the package (and its manifest) by the path the package offers it at |
| Astra 2: one library under two names became two copies in the stage | The bake refuses it by name: one directory is one package |
| Grok 4: alike fonts had to list faces in one order | Faces compare as a set |

## 12. Code review round 3 dispositions

The delta review of round 2's fixes (`llp/reviews/code-2026-10-05-1091-r3.{astra,grok}.md`), both
UNSOUND, both confirming every round-2 input fixed. Every finding is taken, with cases in
`scope_review.rs`; every plan stays byte-identical.

| Finding | Fix |
|---|---|
| Astra 1 / Grok 1: round 2's handler exception kept any whole-attribute call of a bound name, so a binding beat an imported `fn` | The exception is gone: an attribute's call resolves as any call does, as the type checker reads it (a handler's bound head is only kept when no `fn` or shape of the name is in scope, which is when the checker binds it too) |
| Astra 2 / Grok 2: a bare number fills the count and `none` the fill mode | The shorthand reader now mirrors `motion`'s `Animations::grammar` step for step: its split, its time, easing, count, direction, fill and play slots, its `is_name` |
| Grok 3: another file's binding renamed a `fn t` or `fn pending`, so its own calls left the intrinsic | Calls of `pending` and `failed`, and of `t` where no binding is, are never renamed: the type checker reads them first |
| Astra 4: a root shape named like a roster function captured a library's roster call | A declaration named like a compiler call is renamed in any file, the root included, when another file calls the name without declaring or naming it |
| Astra 3 / Astra 5 / Grok 4: copying package files into the stage split identities (an export that links inside the package), broke relative uses from a linked entry, dropped hidden directories, and staged a linked directory under one name | The bake no longer copies packages: it links `node_modules/<name>` in the stage to each package's directory, so the staged compile reads the same files by the same paths as the original; two names for one directory are two links to one package |
| Astra 6: creating a missing export target did not rebuild the wasm loop | A target that fails to resolve is in `consulted`, so its creation is a change |

## 13. Code review round 4 dispositions

The delta review of round 3's fixes (`llp/reviews/code-2026-10-05-1091-r4.{astra,grok}.md`), both
UNSOUND, both confirming every round-3 input fixed. Every finding is taken, with cases in
`scope_review.rs` where the compiler is concerned; every plan stays byte-identical.

| Finding | Fix |
|---|---|
| Astra 2: a computed easing function (`steps(${n}, …)`) filled no slot | A part with a computed value fills what its literal text says: an easing function, or a time by its unit; never the name |
| Astra 3: per-token trimming shifted the rename's offsets (a tab, or a wide space, which could panic) | Animations are trimmed and split at spaces alone, as `motion` splits them; offsets are the split's |
| Grok 1: `inf`, `infinity` and `nan` are numbers to `motion` | Numbers parse as Rust parses them, as `motion` does |
| Astra 6 / Grok 2: any binding named `t` stopped the strings intrinsic | Only an action, prop or inject of the name does, as the type checker reads it |
| Astra 4: an imported style of a roster name shielded a root shape | Only a use that names a shape or `fn` counts |
| Astra 1: a package inside the app, reached both by path and by name, was two files in the stage | Its stage link points at its staged copy, the one the relative path reaches |
| Grok 3: linking needs a privilege Windows may not grant | Without it, the stage copies the package's Contract files and manifest |
| Astra 5: the TypeScript module producer of `host/web/dev.mjs` watched no package; its compiler-input watcher skipped `node_modules` | Both watch the package sources `contract sources` lists |
| Astra 7 / Grok 4: a missing directory crashed the JS loop's watcher | Only existing directories inside a package or `node_modules` are watched for a missing file |
| Astra 8: installing a missing package did not rebuild the wasm session | Where an install would put its manifest is watched |
| Grok 5: a package's dot directories did not live-reload | A package watcher skips only its `node_modules` |

## 14. Code review round 5 dispositions

The last round Charlie allowed (`llp/reviews/code-2026-10-05-1091-r5.{astra,grok}.md`), both
UNSOUND. Astra 1 was a regression of round 4's own fix; every finding is taken, with compiler cases
in `scope_review.rs` and the bake's cases driven end to end on macOS. No sixth round was run: what
round 5's fixes themselves introduce is unreviewed.

| Finding | Fix |
|---|---|
| Astra 1: round 4 linked a package installed under the app's own `node_modules` to itself in the stage | Only a package the capture copied (in the app, outside its `node_modules`) is linked to the staged copy; an installed one is linked where it lives. Driven: an app whose library is a real directory in its `node_modules` bakes and runs |
| Astra 2 / Grok 1: Windows' copy fallback made one declaration two again | No copy: a junction, which needs no privilege, else a refusal that names the package |
| Astra 3 / Grok 2: `t(…)` was decided by the component's bindings, not the innermost one | Each binding records whether it is an action, prop or inject; the innermost `t` decides. Bindings enter scope in the checker's order (props and injects, each state, then the rest); a state initializer naming an action is the checker's refusal (`type-initializer-scope`) |
| Astra 7: a third time made `motion` refuse a shorthand the rename then made valid | The reader refuses exactly where `motion` does (a third time, a second name, a word that fills nothing), and a refused shorthand is not rewritten |
| Astra 4: a failed TypeScript generation installed no package watcher | The producer re-reads the graph after every generation, failed or not |
| Astra 5: a linked package whose manifest refused was not watched | A consulted manifest's directory is watched as a package |
| Astra 6 / Grok 3: a missing package was looked for at four ancestors; a fresh app with no `node_modules` did not see its install | Every ancestor is recorded; the JS loop watches the directory where a missing `node_modules` would be made |

## 15. Code review round 6 dispositions

Charlie authorized up to ten more rounds after round 5 ("let's get this right"). Round 6
(`llp/reviews/code-2026-10-05-1091-r6.{astra,grok}.md`): Astra UNSOUND, Grok SOUND WITH CHANGES.
Every finding is taken; every plan stays byte-identical.

| Finding | Fix |
|---|---|
| Astra 1: a package re-exporting another split a declaration in the stage (a staged copy and its original); Grok 2: the Windows junction command split on `&` | Structural: **the TypeScript bake compiles the app's Contract where it lives**, as the web build does, and checks the capture after the compile, so the plan is built from the bytes captured or the bake is refused. The stage holds the TypeScript alone; the package links, the staged-copy rule, relocation and the Windows junction are deleted. This supersedes §5's and §12–§14's staging |
| Astra 2: `canvas surface=chart()` named a function | A `surface` call's head is the drawing module's; only its arguments are rewritten |
| Grok 1: a task named `t` hid a prop `t` | Tasks are not bindings (the checker's scope holds none) |
| Astra 7: 256 times overflowed a `u8` | The count saturates |
| Astra 4: relinking an install to another directory did not rebuild the wasm session | The install's own `package.json` path is watched as well as its real one |
| Astra 3: a watch root that is a link stopped the TypeScript producer | Package roots are watched at their real paths; one that cannot be watched is said, not a stop |
| Astra 5: the producer missed an install where no `node_modules` was | It watches the directory where that `node_modules` would be made |
| Astra 6: Completion Storm's build scripts did not track used files | They call `contract::rerun_if_changed` |

## 16. Code review round 7 dispositions

Round 7 (`llp/reviews/code-2026-10-05-1091-r7.{astra,grok}.md`): both UNSOUND, both confirming
round 6's inputs fixed. Every finding is taken; every plan stays byte-identical.

| Finding | Fix |
|---|---|
| Astra 1 / Grok 1: round 6's `surface` exception also kept a component argument's or another element's `surface=f()` head | Only on `canvas`, the one tag that owns `surface` |
| Astra 2: the bake's recheck compared package names and roots, not their bytes | The bake reads every Contract source and consulted manifest the compile reads, before and after, and refuses on any difference |
| Grok 2: the bake still refused two versions of one package, a rule of the links | Gone with them |
| Astra 3: a link retargeted at a twin (same time and length) did not rebuild the wasm session | Its stamp has the file's inode and device |
| Astra 4: retargeting an exported link inside a package went unseen | The export's offered path is watched; the TypeScript watcher reads a link as its target and the target's metadata |
| Astra 5: a nearer install than the one resolved went unseen | Every nearer candidate is watched even when a farther one resolves |
| Grok 3: the TypeScript producer's missing-`node_modules` watch assumed `/` | Either separator |
| Found while verifying: the nearer candidates reached `build.rs` as `rerun-if-changed` paths that did not exist, so Cargo reran every build and the driver called it stale | `contract::rerun_if_changed` lists only paths that exist (a failed build reruns its script anyway); the dev loops still watch the candidates. Driven: a second macOS build is a no-op (1.8 s) |

## 17. Code review round 8 dispositions

Round 8 (`llp/reviews/code-2026-10-05-1091-r8.{astra,grok}.md`): both UNSOUND. Both found that
round 7's nearer-install candidates broke deploy: the closure check `realpath`ed them and threw, so
deploying `markdown` or `llp` (whose `@exact/reading` is hoisted) failed on main. Every finding is
taken; every plan stays byte-identical.

| Finding | Fix |
|---|---|
| Astra 1 / Grok 1: deploy's closure check threw on a consulted path that does not exist | It checks only consulted paths that exist (a missing one is where resolution found nothing); `scripts/deploy-paths.test.mjs`, which had required the throw, now requires the opposite |
| Astra 2: component arguments went through the style rows' `class`, `animation` and timeline rewriting | A component argument is a value: only its expression is rewritten |
| Astra 3: a generated name (`val__ui`) could be an action's in its own file | A generated name avoids every binding in every file |
| Astra 4: dropping missing paths from Cargo lost a nearer install | Cargo watches the `package.json` and lockfile beside a nearer `node_modules`, which an install edits |
| Astra 5: a relative use through a link was watched only by its target | Watched by the path written too |
| Astra 6 / Grok 2: a link retargeted at an older file, or an install link retargeted, went unseen by the JS loop and the TypeScript producer | A package's events are edits whatever their time; both loops watch an install that is a link as one entry of its directory |
| Astra 7 / Grok 3: the TypeScript producer's graph filter assumed `/` | Path tests by `path.relative`, separators either way |
| Astra 8 / Grok 4: off Unix, a stamp had no file identity | It carries the file's creation time |

