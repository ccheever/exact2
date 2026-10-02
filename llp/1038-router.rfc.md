# LLP 1038: Router — a location, a stack per tab, six verbs

**Type:** RFC
**Status:** Accepted by Charlie, 2026-09-14 — every §10 question ruled; slices 1 and 2 implemented and integrated. The held-contact and iPad verification follow-ups in §9 remain; slice 3 is deferred to its first consumer. r2 (r1 was `1037-router.rfc.md`, Claude (Opus 5), 2026-09-14, superseded and withdrawn the same day; 1037 is the DartNative research, allocated four minutes earlier — numbers are never reused). **The rulings:** a web tab switch is a history entry (D12; §10 Q1); a template literal passed to a verb is refused — a location is a literal, a `path()` call, or a variable (D3; §10 Q2); the viewport fact the rail needs is LLP 1039 (D12); no restore in v1, kept easy to add (§7; Q3); LLP 1001 and 1012 move to `llp/foundation/` so this and 1039 join the working set (Q4); Astra implements, 1039 first (Q5)
**Systems:** a new leaf crate `exact-route` (the value, the verbs, matching, the canonical location); Contract (a `routes` declaration, four compiler-declared shapes, roster reads and verbs, `path()`); Plan (a `routes` table, the router slot in the header, baked resource arguments); Runner (the launch location as a boot fact, the slot filled at boot, the compiled-value rule); Web host (address bar, session history, the launch location, serving fallback); Apple and Linux hosts (URL entry points, the hide-and-inert rule, the push-or-swap rule); TypeScript sources (the same crate behind three functions); Agent API (`state.navigation.url`, a URL as a form of `type`)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Implementer:** Astra (`gpt-6-astra` through Codex), assigned by Charlie 2026-09-14. Slice 1 (§9, chunks a–e) landed in `248f322`/`887caa6`, `3ff55a0`/`aac382c`, `63cf660`, `319d847`, and `2a3d793`/`e476354`, with follow-up fixes through `330177a` and the S6–S11 review fixes here. Slice 2's web history/serving and native URL lanes were integrated through `5444575`; Codex prepared delivery and addressed review findings on 2026-09-15. The TS binding (slice 3) waits for its first source consumer.
**Date:** 2026-09-14
**Related:** LLP 1035.001 D1 (the route props, the Back control), D2 (the three outcomes), D6 (refusals are journal lines), D10 (the Router's semantic core — this RFC is the "later contract decision" its last bullet names); LLP 1005 §5–§8 (roster, boot, settlement, row slots); LLP 1006 (the compiler); LLP 1007 §4 (the glue), §6 (reload carries state); LLP 1012 (the eight operations); LLP 1017 P4c (per-instance state, and why a row holds no resource), P7 (tests in the language); LLP 1018 (the store); LLP 1023 §3 (the URL is the app); LLP 1027.001 §2 (Contract owns UI state; sources receive and return values), D1 (the standard utility surface); LLP 1030 D7 (a fact the runner answers itself); LLP 1031 D5 (navigation is an intention); LLP 1039 (the viewport fact the rail needs); `rules/DEFERRED.md` (no interactive-navigation model, no route payloads, no platform-suffixed routes). Research, never authority: exact1 LLP 0010, 0051, 0282, 0290, 0305, 0310, 0311, 0494; `~/projects/exact/router-core` and `packages/exact-router/src/browser.ts`

## Summary

Charlie, 2026-09-14: the Router should be "very focused on its core of just
keeping track of routes and URLs, in the abstract"; it should work across
Rust and TypeScript; presentation is the app's most of the time; and routing
should not need to differ per platform. On a big desktop screen Interview's
bottom tabs should become a left rail, as Twitter's do.

The core is one **value** and six **pure functions** over it, specified in
§3 so that a Rust crate, its TypeScript binding, and a Contract test all
answer from one fixture corpus. A screen's identity is its **location** — a
URL path and query, canonical by the browser's rules — and the same string
names the same screen on every host. Routes are declared once (§4 D2):
patterns in the `URLPattern` subset the web already spells, nested to say
what sits above what, grouped by tab. The `Router` value holds one retained
stack per tab, the selected tab, and an id counter; it lives in an ordinary
state slot that actions write with `open`, `push`, `replace`, `back`,
`select` and `go`. Nothing else is in the router: no loaders, guards,
payloads, transitions, scroll, or typed-route codegen (§7).

Everything around the core is projection, and each host does one thing.
The app renders `each e in stack(nav)` under its navigation root, so LLP
1035.001 D1's landed prefix rule projects a real history without a new
model; UIKit still owns the swipe. The web mirrors the stack onto session
history and shows the location in the address bar. Every host delivers an
incoming URL as one `navigate` event and passes a launch URL as one boot
fact. Presentation — a pushed screen, a sheet, a rail, a split — is chosen
by the app from the same value; the one built-in platform difference is
that a tab switch is a history entry on the web and not on native (§4 D12).

## 1. Today, in the code

**The stack is a hierarchy, not a history.** A route is a direct child of
the navigation root carrying `navigationKey`; the stack is the prefix of the
declared routes through the selected one
(`host/apple/Sources/ExactKit/NavigationRules.swift:17-20`, the same rule in
`host/web/glue.js:827-830` and `Mac/AgentMac.swift:44-53`). Depth is bounded
by how many route nodes an author writes, in a fixed order. Messages fits
the shape and pays for it with a four-way nested ternary on the root
(`apps/messages/app.contract:494`). A social app does not fit: post → author
→ another post cannot be written, because the second post would need a
second route node.

**Nothing is a URL.** The web host never calls `pushState` and never listens
for `popstate`; `location` is read for `?agent` and for asset origins only
(`glue.js:73,123`). The address bar reads `/` forever; a reload drops the
screen; browser Back leaves the app. The server answers any path but `/`
with 404 (`host/web/serve.mjs:324`, `dev.mjs:863`), and the document loads
its script by a relative path (`index.html:48`), so a deep URL could not
even boot. No host turns an incoming URL into a screen: `openURL` on iOS and
macOS reaches only `ExactDevelopmentLink`, which accepts
`<scheme>://open?url=` and nothing else (`PlanURL.swift:113-136`,
`ExactIOS/main.swift:225-231`, `ExactMac/main.swift:135-136`).

**Apps build page machines.** Interview keeps five slots that name the
screen (`page`, `tab`, `postId`, `selected`, `personId`), thirteen actions
write `page`, and a `scrollReset` counter is decremented in fourteen places
so the one shared scroll node forgets its position between screens
(`~/projects/interview/app.contract:110-119, 259-414`). Because every
screen is a `when page == …` branch inside one route, UIKit's stack is one
deep and the edge swipe correctly does nothing
(`NavigationRules.popMayBegin` requires depth > 1). Rewriting that inside
today's vocabulary would mean one route node per *kind* of level in a fixed
order — and still no repeat.

**Two hosts do not project routes at all.** macOS reads the props only to
answer the agent (`AgentMac.swift:42-56`); Linux answers
`navigation: {unavailable}` (`host/linux/src/agent.rs:124`). Both lay out
every route node as authored; the apps hide their own screens with `when`.
The web hides and inerts unselected routes and keeps the one under a sheet
visible (`glue.js:824-836`); iOS puts them in a navigation controller.

**D10 drew the boundary and left this open.** LLP 1035.001 D10: the core is
"stable route keys, an ordered stack, a selected tab, modal presentation,
and completed/cancelled native outcomes. Application state in the Rust
runner owns those values. Hosts project them and never become a second
router state machine." Per-tab retained stacks and browser URL/history
"require explicit route/session state and a later contract decision". This
is that decision.

## 2. What exact1 taught (research, cited for how it worked)

- **A screen is a URL, on every platform.** exact1 held this from start to
  finish — `RouterState.matches` is location-derived (0010, Invariant 1);
  "parity is a URL story" (0282). Kept, as D1.
- **The router became the application framework.** `router.ts` was 12,706
  lines and about 330 methods on 2026-07-05 (0305 R4); 0494 counts the
  package at 274 files and about 130K lines, "an application framework
  wearing a router's name", and moves loaders, caches, payloads, auth and
  head/meta out of it. Workflows shipped with zero callers, receipts with no
  read surface, navigation contracts with one adopter (0305 R5). Typed routes
  "prevented zero defects that actually occurred" (0290). So: none of those
  here (§7), and params are strings.
- **Owning native navigation graded worst.** exact1 had no
  `UINavigationController` — "the back stack is the router's history array,
  by design" (0305) — and graded "B- iOS execution" against "A- web/macOS"
  (0305). Its own last word points the other way: Google's Navigation 3
  "*abandoned* framework-owned back stacks for app-owned ones — 'you own the
  back stack' — with the framework projecting UI from a plain state list"
  (0305), which is this RFC's shape. exact2 already lets UIKit own
  recognition, progress and cancellation (1035.001 D1/D2); nothing here
  touches that.
- **Twin implementations drift.** "Every drift was found *after* it shipped"
  (0311: `%40` encoding, base-path matching, snapshot eviction, the redirect
  loop guard), which is why `router-core` (10,641 lines of Rust ported from
  the TypeScript) exists at all. Here there is one implementation from the
  first commit and the corpus runs in three places (D9).
- **Identity from a clock is not reproducible.** exact1 minted history-entry
  ids as `entry_<millis>_<sequence>`, so the same trace through the same
  core yielded different ids depending on which thread supplied the clock;
  0566 §8.9's remedy is "stop deriving identity from a clock at all — a
  per-root monotone counter is already half the id". §3's `next` is that
  counter, and a `Router` value is a pure function of its verb sequence.
- **A navigation that silently does nothing is the bug class.** "A
  navigation that does not commit fulfills indistinguishably from one that
  does" (0407) is what the "second click works" reports traced to. Here a
  refused verb leaves the value unchanged *and* journals (1035.001 D6); a
  Contract test sees both.
- **The web projection that worked** listened to committed navigations and
  never patched router methods: push → `pushState`, replace →
  `replaceState`, an in-app pop → `history.go(-k)` with the echo consumed;
  popstates serialized (`browser.ts:248-295, 346-460`; ENG-22397/22399 came
  from the monkey-patching predecessor, ENG-23187 from unserialized
  popstates). D7 is that algorithm, keyed on entry ids.
- **Tabs and history genuinely differ.** "`router.back()` cannot both always
  pop the canonical history and never switch tabs" (0051); native tab
  switches are not history entries, the web's are. D12 keeps one value and
  lets only the projection differ.
- **Cold deep links.** exact1 contradicted itself between the primary tab
  (0010: "the active tab does NOT influence deep-link routing") and the
  active tab (0051: "if none matches, push onto the currently active tab"),
  and synthesized "one parent, not a full synthesized ancestor chain"
  because a full chain "re-runs ancestor loaders the user never visited"
  (0310 §3.3). Nothing here has a loader and parents are declared, so
  `open` builds the declared chain on the declared tab (§3) and is
  deterministic wherever the user was.
- **Content and presentation are different things.** "Navigation content is
  *which routes are active* — a single, canonical, URL-derived match chain.
  It is form-factor-independent. Navigation presentation is *how those
  active routes are arranged and shown*" (0148); "you cannot media-query
  from a bottom-tab navigator to a sidebar navigator, because they are not
  the same component being restyled" (0148). "Resize and rotation never
  mutate the history stack" (0010 Invariant 4). Kept, as D12: the rail and
  the bar render one value.

## 3. The core, in the abstract

This section is the whole router. It names no host, no Contract syntax, no
executor. The Rust crate implements it, the TypeScript binding binds it,
and the corpus in §8 holds every sentence.

**Location.** A string `path[?query]`: an absolute path (`/`, segments
separated by `/`) and an optional query. Never a scheme, host, or fragment.
Its **canonical form** is what a browser yields for
`new URL("https://x" + location)` as `pathname + search`, prefixing `/`
first when the location does not start with it: dot segments
resolved, the path percent-encode set applied per segment, the query's
percent-encode set applied to the query, everything else left as typed.
Prefixing the origin makes every location an absolute path, so `//b` stays
`//b` and canonicalization is idempotent even after dot-segment removal.
Two locations name the same screen exactly when their canonical forms are
equal. `location_of(href)` derives a location from any absolute URL: for
`http`/`https`, `pathname + search`; for any other scheme, `"/"` followed
by everything after `scheme://`, canonicalized — so `interview://post/42`
and `https://interview.app/post/42` are the same location.

**Table.** An ordered list of routes `{name, pattern, parent?, tab?,
notfound?}`. A pattern is a path whose segments are literals or `:name`
(the `URLPattern` subset: no wildcards, optional groups, or regexps until a
second consumer needs one). A `:name` matches one non-empty segment and
binds its percent-decoded value. A `notfound` route has no pattern. A tab
is a route with `tab = true`; every route belongs to one tab (the nearest
tab ancestor, else the first tab); a table with no tab rows has one tab
named for its first route. Static checks (the compiler's rejects, D2):
duplicate names; a pattern fully shadowed by an earlier one; a parent whose
`:name` the child cannot supply; a `:name` that is not an identifier;
no pattern matching `/` and no `notfound` for the boot location.

**Match.** `match(table, location) → {name, params} | none`: canonicalize;
split the path into segments (a trailing slash is an empty last segment and
matches nothing); take the first route in declared order whose pattern
matches segment for segment; `params` has one string field per `:name`
declared anywhere in the table, `""` where the route does not bind it. The
query is never matched; it rides in the entry's location.

**Entry.** `{id: number, name: string, url: string, tab: string, params}`.
`id` is minted from the value's counter, unique within the value's life,
and never reused. `url` is the canonical location.

**Router.** `{tab: string, tabs: [{name, stack: [Entry]}], next: number}`.
Every tab's stack is non-empty and begins with an entry matching that tab's
own root route, whose query or parameters may change through `replace`. The
**selected stack** is the stack of `tab`; its last entry is the **top**.
Reads: `stack(r)`, `top(r)`, `depth(r)`, `params(r, name)` (that `:name`'s
non-empty values over the selected stack, root first, duplicates kept),
`searchParam(entry, name)` (`URLSearchParams.get`, `""` when absent).

**Chain.** `chain(table, location)`: the matched route, its declared
ancestors, and its tab's root, each as an entry whose params are taken by
name from the match and whose `url` is its pattern formatted with those
params (percent-encoded). Root first. For a `notfound` match the chain is
the first tab's root then the `notfound` entry with the unmatched location
as its `url`.

**Verbs.** Each is `Router → Router`, total, and pure. A location no route
matches — and no `notfound` route absorbs — leaves the value unchanged and
reports one refusal (the caller journals it, 1035.001 D6).

| verb | effect |
|---|---|
| `open(r, loc)` | Select the matched route's tab; replace that tab's stack with `chain(loc)`. Positions whose existing entry has the same `url` keep their `id`; the rest are fresh. Other tabs are untouched. A launch, a reload, a notification, a deep link. |
| `push(r, loc)` | Append a fresh entry for `loc` to the selected stack — on the selected tab whatever tab the route is declared under. A post opened from Search sits on Search. A `loc` that is already the top is unchanged: as HTML replaces the entry for a same-URL navigation, a link to the screen shown adds no visit (2026-10-02; RealWorld's Home and feed tabs added duplicates). |
| `replace(r, loc)` | Give the top entry `loc`'s name, url and params; keep its `id`. A search query mirrored into the URL; a redirect. At depth 1, only a location matching the tab's own root route is allowed; another route leaves the value unchanged with one refusal. |
| `back(r)` | Drop the top entry. At depth 1, unchanged. |
| `select(r, t)` | Select tab `t`, showing its retained stack. Selecting the selected tab pops it to its root (the platform convention: `UITabBarController`, Twitter). Unknown `t`: unchanged, refused. |
| `go(r, loc)` | Traverse: if `loc` is the top, unchanged; else if it is in the selected stack, pop to its nearest occurrence (ids kept); else if it is the top of another tab, `select` that tab; else `push`. A browser Back or Forward past one step; a pasted in-app link. |

**Laws** (the corpus is these, over a fixed table):
`stack(back(push(r, u))) = stack(r)` for `u` not the top's url; `push(r, top(r).url) = r`; `back(r) = r` when `depth(r) = 1`;
`select(select(r, t), t)` has depth 1; `open(open(r, u), u) = open(r, u)`;
`top(replace(r, u)).id = top(r).id`; `go(r, top(r).url) = r`; ids in any
reachable value are distinct and `next` exceeds them all; every entry's
`url` is canonical and `match(url)` gives back its `name` and `params`;
`canonical` is idempotent and agrees with Chrome on the fixture set.

## 4. Decisions

### D1 — A screen's identity is its canonical location, the same on every host

§3's location, canonical by the browser's serializer, is what the address
bar shows, what a share sheet copies, what a notification carries, and what
`Entry.url` holds. The crate implements the path-and-query subset of WHATWG
canonicalization itself (dot segments, the two percent-encode sets: a few
hundred lines, no dependency) rather than pulling a full URL parser into
every host's binary; Chrome is the oracle for its fixtures (D9), as it is
for layout and motion.

### D2 — Routes are declared once, in Contract, nested for parents and grouped by tab

```
routes nav
  tab home "/"
    notifications "/notifications"
    post "/post/:post"
  tab prompts "/prompts"
    ask "/prompts/new"
    question "/prompt/:question"
      write "/prompt/:question/write"
  tab messages "/messages"
  tab search "/search"
  tab profile "/profile"
    settings "/settings"
  person "/people/:person"
  notfound
```

`routes <slot>` is a file-scope declaration beside `font`, `shape` and
`style` (`contract/syntax/src/parser.rs:196-230`); it declares the root
slot that holds the value, exactly one per app, refused in a file with no
root component (`analyze-routes-not-root`). Indentation declares the
parent; `tab` declares a tab; the first tab is the default.
A tab route named `tab` is written unambiguously as `tab tab "/x"`. Messages reads
`routes nav` / `inbox "/"` / `  thread "/t/:thread"` /
`    details "/t/:thread/details"` / `      newContact "/t/:thread/contact"`
/ `  compose "/new"`.

The rejects, one fixture each in `contract/corpus/rejects.txt`:
`route-duplicate`, `route-shadowed`, `route-parent-param`, `route-pattern`
(a segment that is neither a literal nor a `:identifier`), `route-no-match`
(a string literal passed to a verb that no pattern matches), `route-unknown`
(`path()` names no route, or with the wrong count of parameters),
`route-template` (a template literal passed to a verb, D3), and `route-root`
(the table must match `/` by pattern or declare `notfound`).

The plan gains a `routes` table (`name`, `pattern`, `parent: opt:routes`,
`tab: bool`, `notfound: bool`) and the header gains `router: opt:slots`,
the same move as the `sources` table and `app_id` (LLP 1005 §1): the plan
is the one declaration authority, and Rust, TypeScript and the hosts read
it rather than restating it.

### D3 — The router is a value in a state slot; the compiler declares its shapes

For an app with `routes`, the compiler declares four shapes into the plan's
`types` table, as if written in the file: `Entry`, `Params` (one `string`
field per distinct `:name`), `Tab`, and `Router` (§3). The slot named by
`routes` has type `Router` and no initializer of its own: the runner fills
it at boot (D5). An action writes it like any slot — `action openPost(id:
string) writes nav` with `nav = push(nav, path("post", id))`. Slot writes
are typed and rolled back on refusal (LLP 1005 §6), the agent's `state`
prints the whole value as typed JSON (`runner/src/agent.rs:345-421`), a
Contract test asserts on it through a derive (`derive url = top(nav).url`;
`expect state url == "/post/42"`, P7), and replaying the same actions
replays the same navigation.

The reads and verbs join the roster (`plan/tables/format.json` `stdlib`,
`runner/src/stdlib.rs`) with parameter and return types that name the
declared shapes and typed lists; today the roster's type vocabulary is
`number | string | bool | any` (`plan/build.rs:208-218`), so it gains the
plan's own type language for these entries and the checker resolves them as
it resolves a `fn` (`contract/types/src/lib.rs:522-545`). Two more entries
under their web names: `encodeURIComponent(string): string` and
`searchParam(Entry, string): string`.

`path(name, args…)` is not a roster entry: the compiler expands it at the
call site into a template of the route's pattern with each `:name`
interpolated through `encodeRouteSegment` (shared with `Table::path`), arity-checked against the table
(`route-unknown`). No per-route codegen, no typed parameter records —
0290's lesson — and no way to forget the encoding. The segment encoder uses
`encodeURIComponent`, but refuses empty, `.` and `..` parameters (literal
`route-unknown`, otherwise a journaled runtime refusal): WHATWG URL parsing
removes even `%2E` and `%2E%2E`, so those values cannot survive as path segments.

A verb's location argument is one of three things (ruled 2026-09-14): a
string literal, checked against the patterns at compile time
(`route-no-match`; `notfound` does not make a literal valid); a `path()` call;
or any expression that is not a template — a variable, a field, the location `navigate` delivered, a string
a source returned — checked at run time and journaled when nothing matches.
A template literal is refused (`route-template`: "use `path()`"), because a
hand-built `` `/people/${id}` `` breaks the moment `id` holds a `/`, `?` or
`%`, and the compiler cannot see what `id` will hold. There is no
`encodeURIComponent` discipline to remember: the encoded form is the only
one the language will write for you, and the un-encoded one cannot be
written inline.

### D4 — Six verbs, as §3 specifies them, and nothing that is not a value

The verbs are §3's, unchanged. A verb is not a command, not an effect, not a
host call; it returns a value the action stores. That is the boundary LLP
1027.001 §2 draws ("Contract/runner owns UI state; data sources receive
values and return values"), and it is why replay, tests, and the corpus are
free.

### D5 — The launch location is a boot fact; bake is `/`; the compiled value is a cache keyed by its arguments

`Runner::boot*` take a `launch: &str` beside `delivery`
(`runner/src/runner/delivery.rs:36-47`, LLP 1030 D7's channel for a fact
the host knows before the first settlement). The runner sets the router
slot to `open(empty, launch)` before slot initializers run, so an
initializer or a resource argument may read it. Bake passes `/`. A host
with no URL passes `/`. The web passes `location.pathname + location.search`.

A reload carries the slot by name like any root slot (`Runner::carry`,
`runner.rs:309-320`; LLP 1007 §6), so editing `app.contract` in the dev loop
keeps the screen. The carried value is re-checked against the new table:
when every entry still matches to the same route name and the tab roster
(names, in order) is unchanged, it is kept; otherwise
the runner keeps `open(empty, top.url)` of the old top, which is a restart
with the address bar honored.

Today a fresh boot takes a resource's compiled value whenever it has one
(`runner/src/runner/settlement.rs:207-217`), which is correct only while
every slot's initial value is the plan's — true until this RFC. So the
`resources` table gains `initial_args: bytes`, the argument values bake
evaluated, and boot takes the compiled value only when the evaluated
arguments equal them; otherwise the source is asked, as after an action. For
every existing app the comparison is always true; for a launch at
`/post/42` the resources that read `nav` are asked and the rest keep their
first frame. This is the general form of the rule LLP 1030 D7 wrote for
`delivery` alone. A source that is not ready at boot answers its compiled
placeholder regardless of arguments, marks it stale, and is asked again with
the current arguments at `data_ready`.

### D6 — Screens are `each` over the selected stack; the landed projection is unchanged; two hosts catch up

```
main navigationKey=`${top(nav).id}` navigationBack="back" navigate=followLink …
  each e in stack(nav) key=e.id
    column navigationKey=`${e.id}` navigationPresentation=(e.name == "ask" ? "modal" : "") …
      Screen(entry=e)
```

A region realizes its roots as children of the enclosing node (LLP 1005
§6: `each` → rows by key in item order), which is what Messages' `when`
routes already rely on; so the rows are direct children of the navigation
root and 1035.001 D1's prefix rule projects the whole stack: `push` adds a
controller, `back` removes one, a repeat is two rows with two ids. Keys are
entry ids, not URLs — 0100's rule that a pop target is "a pinned
history-entry identity … never recomputed from `matches` arithmetic",
applied to the tree. Per-instance state (P4c)
is keyed by the row, so a screen's draft or expanded comment lives and dies
with its entry, and a screen beneath a pushed one keeps its scroll node
mounted — the fourteen `scrollReset`s go. The Back rule stays: the selected
route's `id="back"` control (resolved by containment, 1035.001 D1), pressed
once on a completed pop; an app that forbids leaving a dirty draft disables
it, and `closedby` keeps its meaning on a modal row.

Routes that are not locations remain app state: Interview's account
switcher is a `when accountSheet` route child after the rows, selected by
`navigationKey=(accountSheet ? "accounts" : \`${top(nav).id}\`)`. Only
entries reach history.

Two host changes, both small. **iOS:** `setViewControllers(_:animated:)`
animates whenever the last owner changes (`NavigationIOS.swift:178`); it
becomes a push or pop only when one controller list is a prefix of the
other, and a swap without a transition otherwise. The animation follows the
prefix relationship of the controller lists, whatever verb produced it: an
`open` that extends the current stack animates as a push, one that replaces
it swaps. *Amended 2026-09-26:* a replacement that keeps the stack's root
and puts a new screen on top (a finished capture giving way to what it
wrote) arrives as UIKit's push; only a change of root — a tab — swaps. *Also amended 2026-09-27:* a completed swipe
dispatches Back to the source's replacement when the app replaced the swiped
screen in place while the finger was down (same depth, the source's node gone);
otherwise the replacement was pushed back in as the pop landed. A newly
selected route that replaced nothing still keeps LLP 1035.001 D2's rule. **macOS and
Linux:** adopt the web's rule (`glue.js:824-836`): every route but the
selected one, and the one beneath a modal, is hidden and inert. Without it
the rows would paint on top of each other on the two hosts that never
projected routes.

### D7 — The web mirrors the stack onto session history, keyed by entry id

The runner knows the value before and after every commit, so it says what
changed: a batch whose commit changed the router slot carries one op,
`{"op":"router","top":<id>,"url":"…","removed":[<ids>]}` — the top entry,
its location, and the ids that left the value in this commit — beside the
`command` and `request` ops it already carries (LLP 1007 §4). No host reads
the slot; no host infers intent from a DOM diff; a non-web host keeps the
op only to answer `state.navigation.url` (D11). The glue keeps `written[]`,
the entries it has put into session history, each stamped
`{exact: index, id, url}`, `gone`, the set of removed ids, and `cursor`,
the current index. Fresh boot `replaceState`s index 0 with the first op.
An in-document reboot retains the mirror if its first router op has
`top == written[cursor].id` (the router carried); otherwise it resets to
index 0. A DOM teardown alone never erases the session-history mirror.

| on a `router` op | history |
|---|---|
| `written[cursor].id == top` | `replaceState` if the url changed, else nothing |
| `written[cursor-k].id == top` for the smallest `k ≥ 1`, and every id in the `k` entries above it is in `gone` | `history.go(-k)`, its `popstate` consumed as an echo; `cursor -= k` |
| otherwise (`push`, `select`, `open`, a `go` that pushed) | `pushState` at `cursor+1`, `written` truncated and appended |

A tab switch therefore pushes even when the tab's top was written earlier
(its entries were retained, not removed), and a `back` after a tab switch
pushes the revealed URL rather than walking through another tab's entry —
the way Twitter's web app behaves.

| on `popstate` to index `j` | delivered as |
|---|---|
| an expected echo | consumed |
| `j == cursor-1` and `written[j].id` is the key of the route directly beneath the selected one | a completed pop: press the `navigationBack` control, as Escape on a sheet does (`glue.js:1486-1493`) and as a finished swipe does (1035.001 D2). If the commit selected that key, stamp the accepted entry with its id and URL; otherwise use the refusal/redirect rule below |
| anything else (Forward, a multi-step Back, a tab switch undone, an entry this page did not write) | the root's `navigate` handler with `written[j].url` (or `location.pathname + search`). If the commit's `router` op lands on that url, `written[j].id` takes the new top's id; otherwise use the refusal/redirect rule below |

A commit with no router change is a refusal: `history.go(cursor-j)` restores
the entry, consumes the echo and journals once. If the router changed but
did not accept the requested key/URL, restore the History entry first, then
mirror that op through the ordinary commit table from the restored cursor
(e.g. a handler pushing `/other` writes a new entry with `pushState`).
After Forward, that push truncates the forward tail, so the new entry need
not grow the total `history.length`.

Popstates are handled one at a time; a commit is synchronous in this host,
so a second one waits in a queue rather than interleaving (0311's rule). A
reload is `open` of the address bar — the declared chain, not the
pre-reload stack; exact1 restored from `sessionStorage`, and nobody has
missed it yet (§7). The handler's name is the web Navigation API's event.

**Serving** (LLP 1023 §7): a request whose path names no file and is not
under `/.exact/` or `/__dev/` answers `index.html`; the document gains
`<base href="/">` so its script, assets and module fetches resolve at the
origin root from any path (`index.html:48`, `glue.js:120-131`).
`serve.mjs`, `dev.mjs` and `origin.mjs` share the rule. Before URL
normalization, their shared raw-target guard refuses encoded dot segments,
`..`, backslashes and `%00` with 404 and `Cache-Control: no-store`.

### D8 — An incoming URL is one `navigate` event; before first pixel it is the launch location

`EventKind` gains `navigate` with one string payload — the location — as
`change` carries its text (`plan/tables/format.json`; the next ABI dispatch
kind after `scroll`; LLP 1005 §6). Only the navigation root may
carry the handler (`lower-navigate-root`). The app's action chooses the
verb: `open` for a notification, because it is deterministic wherever the
user was (0010's reason); `go` for a link. Sources: iOS
`scene(_:openURLContexts:)` and `connectionOptions.urlContexts` for a
universal link (`applinks:`, `associatedDomains` in `app.json`) or the
app's own scheme (`urlSchemes`); macOS `application(_:open urls:)` for a
non-file URL; Linux, the first non-flag argument; the web, D7. A URL that
arrives before boot is the launch location instead. `ExactDevelopmentLink`
keeps its `open?url=` form and never reaches `navigate`; `DocumentsMac`'s
test-id seam is for files, not locations. The location is derived by
`location_of` (§3) in the crate, so the hosts agree.

### D9 — One implementation; the value crosses the seam; the corpus runs in three places

- **`exact-route`** is a leaf crate: the table, `canonical`, `location_of`,
  `match`, `chain`, `path`, the six verbs and the reads over plain structs
  with `serde`. It depends on nothing but `serde`. The runner's roster
  entries call it, converting to and from the plan's positional `Value` by
  the declared shapes. The compiler calls it to validate patterns and
  literals, so validation cannot disagree with run time.
- **Values cross the seam unchanged.** `Router` and `Entry` are plan
  shapes, so a resource or mutation argument of type `Entry` reaches a Rust
  or TypeScript source exactly as the source's own shapes do (LLP 1027.001
  §2); the generated `app.contract.d.ts` already names every plan type
  (`contract/cli/src/typescript.rs:13-95`).
- **The same functions in both languages.** A Rust source calls
  `exact_route::{match, path, canonical}` with the plan's table (it has the
  plan at `bind`). A TypeScript source calls `exact.routes.match(url)`,
  `.path(name, params)` and `.canonical(url)`, bound through the pure door
  that already binds `URL` (`js/src/pure.rs:7-93`, op 7; `js/src/pure.js`),
  and in the browser through the executor wasm that links the same crate
  (`js/web/src/lib.rs`). A source never navigates: it returns a location
  and an action applies a verb (1027.001 §2).
- **One corpus.** `exact-route/tests/corpus.json`: tables, verb sequences,
  expected values, canonicalization pairs. It runs in `cargo test -p
  exact-route`; through the TS binding under Hermes and in Chrome, the way
  `js/tests/pure.rs` runs the URL fixtures; and the canonicalization pairs
  are generated from Chrome's `URL` and committed, the way
  `host/web/tests/fixtures/browser-motion.txt` is.

### D10 — A screen beneath the top gets its data from a root resource keyed by the stack

Data is the cost of repeats. A screen that is not the top must still render
while a swipe reveals it, and only the root holds resources
(`type-child-resource`; LLP 1017 P4c — "a row must not open N requests").
The idiom is `params(nav, "question")`: a root resource takes the list of a
parameter's values over the selected stack, the source answers a list, and
each screen picks its record by `e.params.question` — the shape Interview's
post reader already has (`each post in data.posts` + `when post.id ==
postId`). One request per commit that changes the stack; a source may
cache. Row resources stay refused: reopening P4c's rule needs a measured
Interview request count or latency this idiom cannot meet, and its trade
named.

### D11 — The agent observes the location and delivers one as a form of `type`

`state.navigation` gains `url`, the last `router` op's location (D7),
beside `route` and `stack`, which stay read from the tree (LLP 1012 §1;
1035.002 D2); Linux keeps `unavailable`. The value itself is already under
`state.slots`. Delivering a URL is `type
<navigation root> "/post/42"`, which calls `navigate` as `type` on an
input calls `change`. On the web, browser Back and Forward are `tap
<navigation root> {"history": -1}`, which calls `history.go` so the real
`popstate` path runs; elsewhere it replies `unsupported`, as `cancel` does
on macOS. No ninth operation.

### D12 — Presentation is the app's; the one platform difference is tab history

The viewport fact for this presentation is LLP 1039’s `exactViewport`: width and height are host boot facts, refreshed on resize. The rail branches on width without changing route state or introducing a router-owned size class.

The router never learns how an entry is shown. From one value:

- **Phone:** each entry is a pushed screen; `ask` is a sheet
  (`navigationPresentation="modal"` on its row).
- **Desktop rail** (the Interview ask): the rail is `button`s reading
  `nav.tab` and applying `select`; the content column renders `stack(nav)`
  exactly as a phone does. Choosing a rail over a bar is a layout decision
  and needs a viewport-width fact Contract does not have. That fact is LLP
  1039: `resource viewport = exactViewport() as shape Viewport`, answered by
  the runner and re-answered in one commit on resize, exactly as
  `delivery` is (`set_delivery`, `runner/src/runner/delivery.rs:57-72`);
  `when viewport.width >= 900` is the media query. 1039 D6 carries the two
  host notes the rail needs — `aria-orientation`, and the macOS D10
  projection leaving a vertical tablist alone — and §4 there shows the rail
  as a sibling of the route rows, the way today's bar is.
- **Split view:** a list column renders the root entry and a detail column
  renders `top(nav)` — one state, two presentations.

On native, `select` is not a history entry: the swipe never changes tabs
and there is no Back button that could. On the web it is a `pushState`
(D7), because browser Back undoing a tab switch is the web's convention and
the address bar must change. Values, verbs and URLs are identical on every
platform; an app that wants different routing on one platform branches its
own actions, and nothing here encourages it.

## 5. Interview, as it would read

```
routes nav                          // D2's table
component Interview
  action openPost(id: string) writes nav
    nav = push(nav, path("post", id))
  action openPerson(id: string) writes nav
    nav = push(nav, path("person", id))
  action openQuestion(id: string) writes nav
    nav = push(nav, path("question", id))
  action back writes nav
    nav = back(nav)
  action selectTab(name: string) writes nav
    nav = select(nav, name)
  action followLink(url: string) writes nav
    nav = go(nav, url)
  action signOut writes outcome, nav
    send outcome = command("logout", "", "", "")
    nav = open(nav, "/")
  action tick writes …, nav
    …
    if current.name == "ask" and not pending(outcome) and resultId != ""
      nav = replace(nav, path("question", resultId))   // the published prompt takes the sheet's place
  derive current = top(nav)
  resource data = app(started, params(nav, "question"), params(nav, "person"), params(nav, "post"), queryFilter, querySearch, revision, …) as shape App
  view
    main navigationKey=(accountSheet ? "accounts" : `${current.id}`) navigationBack=(accountSheet ? "close-account-sheet" : "back") navigate=followLink …
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "ask" ? "modal" : "") …
          Screen(entry=e)               // header, content by e.name, tab bar
      when accountSheet
        column navigationKey="accounts" navigationPresentation="modal" …
```

`page`, `tab`, `postId`, `selected`, `personId`, `scrollReset` and the
fourteen resets are deleted; `effectiveQuestion`, `queryFilter` and
`querySearch` derive from `current`. Opening an author from a post pushes
the profile onto the current stack; Back returns to the post; the feed
keeps its scroll position under it because its row stays mounted. On the
web the same actions produce `/`, `/post/42` and `/people/7` in the address
bar, browser Back walks them, and a reload of `/prompt/5/write` boots with
`[prompts, question 5, write]` on the stack.

## 6. Messages, as it would read

The nested ternary at line 494 becomes the six-line `routes` in D2 and
`navigationKey=\`${top(nav).id}\``; `open`, `back`, `showDetails`,
`closeDetails`, `newMessage`, `cancelNewMessage`, `createContact` and
`discardContact` become verb applications on `nav`; each screen names its
own control `id="back"` (containment already scopes it, 1035.001 D1), so
`navigationBack="back"` is constant. The draft, reply and selection state
those actions also touch is not navigation and stays exactly where it is.
`thread` becomes `top(nav).params.thread`. Nothing in the projection
changes: the same nodes carry `navigationPresentation`, `navigationSource`
and `closedby`.

## 7. What this deliberately does not add

Each with the trigger that would earn it back:

- Loaders, caches, prefetching, suspense, route payloads (DEFERRED's
  server-generation line; 0494). Never.
- Auth guards or redirect tables: an action checks `data.authenticated` and
  applies `replace`. Never.
- Transitions, a gesture model, progress values (1035.001 §6; DEFERRED
  §Motion). Never.
- Typed-route codegen, receipts, navigation contracts, workflows (0290,
  0305 R5). Never.
- File-based routes, layouts as files, platform-suffixed routes
  (DEFERRED). Never.
- Named outlets or parallel routes: a split view is two columns over one
  value (D12). An app whose second column is not a function of the first.
- Wildcards, optional segments, regexp patterns. A second consumer that
  cannot spell its URLs without one.
- Router-owned scroll restoration: mounted rows keep their own scroll. A
  measured case where a remounted row must restore a position.
- ~~Retained *views* for unselected tabs (their entries are retained, their
  rows are not): a measured Interview complaint about a tab forgetting its
  place. It needs one navigation owner per tab in the Apple projection.~~
  Built 2026-10-02 (LLP 1075.003 §3.7, James's app the consumer): a root's
  tablist names its tabpanels with `aria-controls`, each panel holds one
  tab's stack, and every tab's rows stay mounted. The web and AppKit hide and
  inert the unselected panels (D6); UIKit gives each tab its own navigation
  controller under a tab bar controller, or under the app's own container
  through the module's `tabContainer` hook.
- Restoring the pre-reload stack or the last session's location: a host
  could pass the last `router` op's url as the launch location under a
  manifest key, with no router change. An app that asks. Ruled no for v1
  (Charlie, 2026-09-14: "we may want to make it easy for people to do this
  but for now I don't think it's important") — so the launch-location seam
  stays the one place it would plug in, and nothing in D5 or D7 assumes the
  stack is never restored.
- ~~Same-origin `link href` interception on the web.~~ Built 2026-09-23 (the
  2026-09-22 review): a plain primary click (no modifier, no `target` or
  `download`) on a same-origin `a[href]` whose location a declared pattern
  matches — never only `notfound` — stays in the document. A link with its
  own `press` navigates by it; any other goes to the root's `navigate`
  handler, as a popstate does, and a refusal journals once. Other clicks,
  origins, fragments of this page and undeclared paths (a file) are the
  browser's; a modified or other-button click on a pressing link runs no
  press (the browser's new tab alone). The match is the plan's table in the
  host (`exact_route_match`); the listener is input glue, after paint.
- A `navigationTitle`: 1035.001 D9 stays the path.

One line goes onto `rules/DEFERRED.md` under Features at acceptance, so §7
binds; nothing comes off it, because none of this was ever on it.

## 8. Verification

| case | where | observation |
|---|---|---|
| §3's laws over three tables (Interview's, Messages', one tabless) and the canonicalization pairs | `exact-route` tests; the TS binding under Hermes and in Chrome; a Contract test through the runner | identical JSON in all three |
| The seven rejects | `contract/corpus/rejects.txt` | one fixture each |
| Launch at `/prompt/5/write`: first pixel, resources asked vs compiled | web (Chrome), iOS (`simctl openurl` before launch), macOS, Linux argv | `state.navigation.url`, stack of three keys, `logs` shows only `nav`-dependent sources asked |
| Home → post → person → post, swipe back three times | iOS, 1035.003 held contact | stack by key at each step; the revealed screen shows its own data while held; the feed's scroll offset intact at the end |
| A cancelled swipe | iOS | stack unchanged, `transition.phase = cancelled`, no `back` dispatched |
| Tab select, re-select, `open` of another tab's URL | iOS, web | iOS: no push animation on a swap, depth equals the retained stack, re-select pops to root; web: one `pushState` per select |
| Browser Back, Forward, a two-step Back, a tab switch undone, reload | web | `location`, `history.length`, `state.navigation.url`; reload gives the declared chain |
| A refused browser Back (Back control disabled) | web | the entry restored by `go(+1)`, one journal line |
| A URL while running | iOS (`simctl openurl`), macOS (`open`), web (`type` on the root) | `open` result; one `navigate` dispatch in `logs` |
| Dev reload with a changed table; with an unchanged one | web dev loop | kept stack / `open` of the old top |
| Interview and Messages on every host | `smoke.mjs` | green, before and after each slice |

## 9. Landing order

0. **LLP 1039 first (Astra).** The viewport fact: about 200 lines on the
   same boot-fact and ABI seams slice 1 touches, and Interview's rail.
1. **Slice 1 — the value, the declaration, the hosts' projection (Astra).**
   `exact-route` and its corpus (Rust); `routes` in the compiler with the
   four shapes, the seven rejects, `path()`; the plan's `routes` table,
   header slot, `initial_args`; the roster's reads and verbs and the two
   web-named entries; the launch fact and the boot rules (D5); D6's iOS
   push-or-swap rule and the macOS/Linux hide-and-inert rule; the `router`
   op, the web's address bar (`replaceState` of its url only), launch
   location and `<base>`; `state.navigation.url`. Interview moves to `routes`; the
   verification rows for the corpus, rejects, launch, swipes and tabs.
   chunk (a) landed 2026-09-14: `exact-route`, 226 corpus cases.
   chunk (b) landed 2026-09-14: plan routes/header/cache keys and typed roster; runner launch, conversions, carry, `take_router_change()` beside unchanged kernel receipts, and corpus replay through the VM.
   chunk (c) landed 2026-09-14: Contract routes grammar, root slot and four positional shapes, table/header emission, typed roster reads and verbs, encoded `path()` expansion, all nine compiler rejects, Interview fixture and runner tests including 66 shared corpus steps; LLP 1006 updated. Host projection and app migration remain chunks (d)/(e).
   chunk (d) landed 2026-09-14: coalesced router ops in web/Apple batches
   and retained on Linux; web launch pathname/query, current URL on module
   reboot, replaceState-only address bar and origin-root base; navigation.js
   split (boot count 1 → 2); iOS prefix-only push/pop versus immediate swap;
   macOS/Linux hide-and-inert projection and agent URL observations (Linux
   stays unavailable). Asked sources are named in the existing journal so
   the launch row distinguishes queries from compiled resource values.

   **Decided (chunk d):** Linux has no foreign batch consumer, so it retains
   `RouterChange` at boot/commit rather than adding a batch mirror. Native
   launch remains `/`; its URL is asserted from Linux's router value while
   `state.navigation` remains unavailable. The web's unmatched-key case now
   preserves projection and journals, bringing its implementation up to
   1035.001 D6. The new web module joins the existing static-file allowlist,
   build copy and source fingerprint; serving routing is unchanged. Host
   drives use the existing routes fixture baked by its test data source;
   temporary `loadQuestions` support in the Caltrain carrier is restored
   before checks and commits, with no app migration. UIKit's second
   `willShow` during cancellation keeps the interactive source until `didShow`;
   the cancelled stack stays unchanged, the phase is `cancelled`, and no Back
   is dispatched. Physical input used the existing pointer helper mapped from
   Simulator's AX `iOSContentGroup` and the agent's screen bounds after the
   driver's hover calibration was unavailable. The existing iOS canvas smoke
   now crops at that reported screen origin; treating both safe areas as a
   title bar had shifted the crop down 34 points (76.65% differing pixels;
   the same capture at the correct origin differs by 0.73%, within its 1% band).

   **Observed (chunk d):** `/tmp/lane-router/1038d/` holds the drives and
   check logs. Web deep launch reports `/prompt/5/write`, keys `1,5,6`, and
   `query questions: loadQuestions`; native launch is `/`. iOS cancellation
   preserves `0,5,6,7` with phase `cancelled` and zero Back calls; three
   completed swipes yield `0,5,6`, `0,5`, `0`, with three Back calls. Tab
   select/restore/open stayed `idle` under platform timing; re-select popped
   to root. Mac at 1280 × 900 and Linux report hidden/inert retained routes
   and refuse their controls; only the modal's immediate underlay is visible
   but inert. Existing `when`-selected routes still project on all four hosts.
   The nested-document dev drive loads root-relative modules/assets and dev
   generation payloads, preserving `?agent=1` and a controlled clock, using
   browser document interception without adding a serving fallback.


   chunk (e) implementation landed 2026-09-14: Interview on `routes` —
   4 slots and 0 actions deleted net (33 → 29 slots, 46 → 46 actions),
   all 14 scroll-reset writes removed. `Screen(entry: Entry)` owns drafts,
   editor and chrome state; shared values/actions use `provide`/`inject`.
   Root SQLite reads answer stacked question/person/post IDs, with keyed
   detail/profile lists and stacked posts outside the filtered feed. The
   pre-replica backend still receives the first scalar IDs; list arguments
   remain owed in Interview’s own LLP. The compiler’s stale child-state
   scope for curried action arguments is fixed, with origin spans preserved
   and an `instance-args.contract` dispatch/diagnostic regression fixture.

   **Decided (chunk e):** Repeated route IDs share one data record in
   first-occurrence stack order so `each d key=d.question.id` stays unique;
   screens retain independent entry keys and local state. Root mutations
   retain their submitting entry ID so a new composer cannot inherit an old
   success notice. No `navigate` handler, serving fallback or history mirror
   is added. The web launch evidence harness only serves the app document at
   the requested deep pathname; it does not implement slice 2 serving.

   **Observed (chunk e):** `/tmp/lane-router/1038e/report.md` records all
   evidence. The Rust/runner corpus and six Contract router tests pass;
   all nine rejects pass. Interview’s three native replica, six adapter and
   five Node tests pass; Exact’s five checks and Caltrain web/macOS/iOS smokes
   pass. Web traverses keys `6,7,8,9` through Home/post/person/post with real
   SQLite records and restores the retained stack on tab select. iOS restores
   Settings depth 2 with 16 `idle` samples, re-selects to depth 1, and cancels
   a held edge swipe with `4,6` unchanged, phase `cancelled`, zero Back calls.
   Web/macOS show bar at 420 × 900 and rail at 1280 × 900. Native launch is `/`;
   Linux reports it through `nav` while navigation remains unavailable.

   **Decided (F9–F11, orchestrator, 2026-09-14):** A deferred source with a
   compiled value answers that stale placeholder at boot regardless of
   argument mismatch, then answers current arguments at `data_ready`; this
   closes a slice-1 rule gap. F10 uses the static routes fixture because
   Interview’s native agent mode withholds SQLite. The F11 browser harness
   pauses the deferred module until first-pixel capture and clears its driver
   query before wasm boot, retaining the controlled clock while allowing the
   ordinary SQLite path; no agent storage policy or serving behavior changes.

   Slice 1 implementation landed 2026-09-14: chunks (a) `248f322`/`887caa6`, (b) `3ff55a0`/`aac382c`, (c) `63cf660`, (d) `319d847`, and (e) Interview `2a3d793`/`e476354` with compiler fix `1741d80`; F9 `0c078e3`, F10 fixture `8fa89a7`/`8265f5b`, and Interview F11 evidence note `5ec1dd9` follow. §8 rows 1–2 pass in Rust/runner/Contract (17 route tests, six Contract router tests, 66 shared steps, nine rejects). Row 3 now passes on Interview web: `/prompt/5/write`, keys `1,5,6`, first pixel 68.8 ms with the module held; `data_ready` asks `app(false,["5","5"],[],[],"Latest","",0,0,0)` and saved SQLite question 5 appears after sign-in/sync; native launches remain `/`. F10’s iOS static fixture taps Home/post A/person P/post B through keys `0` → `0,5` → `0,5,6` → `0,5,6,7`, displaying their own titles/name, after scrolling feed node 22 to 240; row 4’s three held swipes and restored offset remain unverified because other desktop apps repeatedly cover Simulator and the pointer helper refuses the contact. Interview row 5 preserves `4,6`, phase `cancelled`, zero Back calls; row 6 restores Settings depth 2, re-selects to depth 1, and samples swaps/open 16 times each at `idle`; Interview web paths follow the top using slice-1 `replaceState`. Evidence and final checks are in `/tmp/lane-router/1038e/followup-report.md`. Remaining work is the unobscured fixture contact drive, Interview row 4 with a seeded agent-mode replica, iPad-wide iOS, backend list arguments, and slice 2’s Messages/history/serving/native URL work; slice 1 is not declared fully verified while row 4 is blocked.

2. **Slice 2 — the web's history and the URL entry points, two lanes.**
   Lane a: D7's mirror and `popstate`, the serving fallback, the browser
   rows. Lane b: the `navigate` event on every host with the `type`/`tap`
   forms (D8, D11), the URL-while-running row. Messages moves to `routes`
   in whichever lands second.
   Lane a implementation (2026-09-14): `navigation.js` implements D7's two
   tables and queues traversals through restoration echoes; Escape and a
   completed browser pop share the selected route's Back control. D11's
   history tap uses `history.go` on the web and replies unsupported in every
   other agent carrier. `serve.mjs`, `dev.mjs` and the published directory
   origin share an extensionless document fallback with reserved-path/file
   precedence; publication replaces the bake's root base with the immutable
   release prefix. LLP 1007 §4, LLP 1023 §7 and LLP 1012 §1 record the landed
   paths. The host-side `navigate(location)` seam journals its explicit
   lane-a refusal and restores: Forward, multi-step Back and undoing a tab
   switch await lane b's one-line event dispatch replacement. Their successful
   URL-handling rows are not claimed by this lane.

   Lane b implementation (2026-09-14): `navigate` is EventKind 13 and ABI
   dispatch kind 14 on Apple/web (Linux dispatches the Rust event); one string
   or no action parameter, first navigation root only (`lower-navigate-root`).
   Web exposes synchronous `exact.navigate(location)`, returning the applied
   batch; lane a owns its popstate caller. Native launch locations now reach
   initializers and first settlement, including prepared plans. Apple derives
   them through `exact_location_of`; iOS URL contexts and browsing-web activities,
   macOS non-file Launch Services URLs, and Linux argv supply them. Explicit
   development links remain separate. The driver accepts a native scheme/path
   through `--url` for cold launch. Manifest schemes reach both Apple plists;
   associated-domain arrays reach the iOS entitlement (the boolean origin form stays).

   Messages moved here: the six-line routes table, one keyed row per entry,
   the selection ternary removed, and constant `navigationBack="back"`. The eight
   named actions plus forward/send/save-contact write `nav`; `open` and `back`
   retain their action names. `followLink` uses the `open` verb. `thread` reads
   the top's parameter; while composing, `chatThread` reads the entry beneath
   the sheet so its conversation resource, reply timer and forwarding editor
   keep the underlying conversation.
   Draft, reply and selection slots remain application state; presentation,
   source and close-policy props stay on the same screen nodes.

   Evidence: `/tmp/lane-router/s2b/`. Row 9 passes with one `navigate` on web
   root type, macOS `open`, and iOS `simctl openurl` on the carrier's device
   (multiple simulators were booted; the OS Open consent was accepted).
   Linux root type also passes. Native cold driver launches show `/post/42`
   in the initializer and first frame, with zero navigate dispatches; macOS
   also passes a real cold Launch Services URL. Scheme/entitlement generation
   is exercised in `manifest.json`. The routes corpus ran in a temporary
   Caltrain carrier with an isolated bundle id and three fixture data replies;
   those carrier source/manifest edits were restored before checks or commit.
   Messages' iOS/macOS smokes and web app-only smoke pass. Full web smoke refuses
   an unrelated bare-plan fixture with `module reload requires a paired generation`.
   Direct iOS/web drives
   additionally cover draft/Back/focus and the root event; web covers empty
   Escape and populated forwarding refusal, including a reply arriving beneath
   the forwarding sheet without changing its draft or active-thread read state.
   The macOS viewport fixture caught the deferred boot taking the content rect;
   retaining `session.viewportSize` restores the full cover viewport. The original and migrated macOS
   Contracts both hit the pre-existing painted-agent-popover obstruction
   (`QUEUE.md`); no held-contact sweep is claimed by these drives.
   The five checks pass with development trust and default `TMPDIR`: build,
   752 workspace tests, clippy/format, staged caps and boot imports. Web's
   app-only assertions also exit zero through a caller that exits after the
   smoke module completes; its natural-exit pipe issue is recorded in `QUEUE.md`.

   Integration (2026-09-14): lane a, then lane b, merged after the slice-1
   review fixes. The glue's history callback now calls `exact.navigate`;
   the mirror still requires the synchronous commit to land on the requested
   URL and restores otherwise. The routes corpus uses `navigate=followLink`
   with `go`. The browser sweep asserts one handler dispatch and the landed
   URL for Forward, multi-step Back and tab undo, and retains refusal/restore
   coverage for both Back and a handler that does not land.

   Delivery review (2026-09-15): Messages' URL handler now saves the outgoing
   conversation's composer and opens the destination's declared ancestry.
   Edited draft/reply overrides belong to a conversation; a separate saved
   composer resource restores a destination on cold launch and browser Forward.
   Cross-thread links clear selection and editing state. Deterministic ancestry
   also prevents multiple retained thread rows from displaying one shared chat
   resource. The app runner regression covers saved drafts/replies, links, Back,
   Forward, details and the compose underlay. The web host retires event
   dispatch for committed removals before applying DOM operations, so removing
   a focused route cannot dispatch a stale blur into a destroyed runner view;
   live and retained editors still receive ordinary blur events.

3. **Slice 3 — TypeScript.** The pure-door binding and the corpus under
   Hermes and Chrome; the Chrome-generated canonicalization fixture. Lands
   with the first source that needs `match` or `path`.

Each slice keeps the five checks and the 1,500-line cap; Interview stays
driveable on web and iOS after every slice. LLP 1039, the viewport fact
the rail needs (D12), is separate and can land in any order.

## 10. Questions for Charlie

1. ~~**Tab history on the web.**~~ Ruled yes, 2026-09-14: a tab switch is
   a history entry on the web and not on native (D12). The alternative was
   `replaceState`, which makes browser Back skip the switch and drops the
   other tab's URL from history.
2. ~~**Encoding.**~~ Ruled, 2026-09-14: a template passed to a verb is
   refused; a location is a literal, a `path()` call, or a variable (D3).
3. ~~**Restore.**~~ Ruled, 2026-09-14: none in v1; a reload and a relaunch
   `open` the address bar or `/`. Keep it easy to add (§7's manifest-key
   path).
4. ~~**The working set.**~~ Ruled, 2026-09-14: LLP 1001 and 1012 — the two
   stable specs — move from `llp/current/` to `llp/foundation/` (5 → 7 of
   10), and this document and 1039 take their places. Nothing is archived.
5. ~~**Implementer.**~~ Ruled, 2026-09-14: Astra, 1039 first, then slice 1,
   sequentially; slice 2 fans out to two lanes.

## 11. What this amends, at acceptance

- **LLP 1035.001 D10**, last bullet: per-tab stacks and URL/history are
  decided here. **D1**: a completed browser Back is a completed pop. The
  iOS projection gains D6's push-or-swap rule; macOS and Linux gain the
  hide-and-inert rule.
- **LLP 1005**: the roster's typed entries; the `routes` table and header
  slot; `initial_args` and the compiled-value rule; the launch fact; the
  `navigate` event; the `router` op.
- **LLP 1006**: the `routes` declaration, the four shapes, `path()`, the
  seven rejects; `navigate` joins the handlers.
- **LLP 1007**: D7. **LLP 1023 §7**: the serving fallback and `<base>`.
- **LLP 1008 §9**, **LLP 1015**: the URL entry points; the two projection
  rules.
- **LLP 1012**: `state.navigation.url`; the `type` and `tap` forms of D11.
- **LLP 1027.001**: `exact.routes` on the standard utility surface (D1).
- **`rules/DEFERRED.md`**, under Features: "Router: no loaders, guards,
  route payloads, typed-route codegen, receipts, outlets, router-owned
  transitions or scroll restoration (LLP 1038 §7)".
