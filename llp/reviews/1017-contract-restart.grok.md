# Panel: LLP 1017 Contract, started over — §8, the forks (grok)

- **Family:** grok (xAI): `grok --prompt-file panel-brief.md -m grok-4.6 --reasoning-effort xhigh --disable-web-search --no-subagents --cwd <capsule> --output-format plain`; workdir = the panel capsule (31 files: the authoring capsule of LLP 1017.001/1017.002 — seven Coterie diaries, exact2 LLP 1004/1005/1006/1012/1016, the rules, both app contracts, `rejects.txt`, the tag table, exact1's 0508 excerpt, guide, caltrain twin, todo field report, Coterie's chat route, bubble, and one sidecar, the evidence extract — plus LLP 1017 r2, 1017.001, 1017.002, and the original brief), read-only, no network.
- **Method:** a panel at Charlie's request, 2026-08-30 (LLP 1016 §6's sense: not a refine loop; no verdict binds). One round, mutually visible: each panelist saw LLP 1017 r2 and the other family's sub-LLP; neither saw the other's panel answer. Panel brief sha256 93a84e19c0a4c9e682028e0be06015350731e85eaf28466113716a45dba6a444; LLP 1017 r2 sha256 636e2a108ecf321d5bcce0655cf7c948fb77e4471ce3291dfb0f92fade6f57b3; capsule sha256 bb796924c7da95ccbc045065926e61886b25ee1139d778ecaeee3cdfc3b7d1da. Run 2026-08-30T13:19:51Z → 2026-08-30T13:25:13Z.
- **Disposition:** the fold is recorded in LLP 1017 §10.

---

## Round 1 — position (received 2026-08-30T13:25:13Z, verbatim; the CLI's narration lines before the document marker are dropped)

# Panel: LLP 1017 — the forks (Grok 4.6)

## F1

**Position.** An expression-only `fn` is not enough for what the diaries escaped for. It covers the class LLP 1004's Appendix already called "Already Contract" (`caltrainColors`, `getServiceColor`, `serviceBadge`) and the six `pure(...)` rows in `coterie/coterie-colors.contract-meta.ts`. The first thing it will be asked to do that it cannot is 0103's `scrollToMessage(id)`, shipped `opaque()` — a command, not an expression. Next is 0103's computed member access. Take Sol's `fn` for a palette, a price string, a projection; keep the data crate exporting into the roster (`stdlib.json`, LLP 1004 D2) for anything with I/O, a host effect, or a loop. Sol's Caltrain sample still calls `formatDistance` with no `fn` body (1017.002); that call is the roster.

**Reason.** 1017 §2.2 names the hatch as "computed member access, a scroll-to-key, a DOM effect, a file input" and "three files and a generator run" (0103). 0095 asked for "a semantic scroll-to-key operation." 0098's capture-phase listener is not a function. LLP 1004 D4 put sorting, geodistance, and the schedule in Rust before the app. r2's "first `fn` that wants a loop" is the wrong first failure: the record's first failure is a host effect.

**What would change my mind.** A Coterie port on exact2 whose first missing construct, by the 0329.008 ladder, is a pure projection that cannot be a `derive` or a one-expression `fn`.

## F3

**Position.** `submit` on `input` (and on `form` if that tag lands). `key=` stays. They are different events. No node in the two apps needs both attrs; the language needs both handlers. Login takes `submit`. Caltrain's search hint takes `key=`.

**Reason.** Exact2's six `EventKind`s are `press` / `change` / `hover` / `focus` / `blur` / `key` (LLP 1005 §6, LLP 1006 §2). A `key` carries the web name; `submit` carries none. Caltrain's stations input is `change` / `focus` / `blur` / `key=searchKey` (`exact2/caltrain.app.contract`, hint `last key ${lastKey}`). Labs-todo paved Enter as `FacetInput submit=` (2026-06-18). exact1's guide already lists `submit` among event attrs. Agent `type … key Enter` (LLP 1012 §1) is the driver's synthesis, not the app handler. r2 cites LLP 1008 §5 (inside a field on macOS and iOS, `key` sees editing commands only) — that file is not in the capsule; if it holds, `if k == "Enter"` on `key=` is web-only. P2 is still required for other branches. Combining them would make `key` mean two things.

**What would change my mind.** A host fixture where `key="Enter"` on a focused `input` runs the same action on web, macOS, and iOS.

## F4

**Position.** Hold: `pending()` plus `match` on a mutation's `option<T>`. No `status` construct. Stale-while-revalidate wants the author to keep reading the value and, separately, to ask the flag.

**Reason.** LLP 1016 D3: the resource keeps the value it had; `pending(resource) → bool`; `option<T>` for every remote resource was rejected because it "retypes every read site." Sol requires `pending value` / `ready value` on queries (1017.002 Data) and the Castle sample exclusive-arms `LoggedOut` four times. That is the `async` region's `loading` / `error` / `empty` / `ready` (`coterie/chat-route.contract` 139–176; 0103: "I did not write a single loading boolean"). Exclusive arms make keep-previous opt-in: copy the ready tree into `pending previous`. Under `pending()`, keep-previous is the default and a resource the view never asks `pending(…)` about is a 0278-class warning. Sol also writes "Ordinary expressions may ignore status when stale-while-revalidate is intended" — that sentence is the r2 rule, and it contradicts requiring the arms.

**What would change my mind.** A diary where a missed `pending()` produced a double-submit or a flash that exhaustive `status` would have refused, and the duplicated trees were cheaper than that bug.

## F5

**Position.** Concede `derive` on a child. Live-tree identity: declaration + which use + enclosing `each` keys. Not source address as a reload key. A dev reload carries root slots by name where the type still fits, settled resources whose arguments still match, and the clock (`DEFERRED.md` §Runtime, Charlie 2026-08-28); child slots restart unless they have a stable qualified name that is not a span.

**Reason.** `coterie/message-bubble.contract` already has six `derive`s on the child (`receiptMark`, `bubbleColor`, …). exact2 inlines children (LLP 1006 §3); a child `derive` is an alias at inlining, as r2 says. I refused it as `analyze-child-resource` and asked Charlie (1017.001 Q6) — delay, not a cost argument. LLP 1005 §6 already keys `each` rows; I inlined singleton uses to renamed root slots. r2's "use address" distinguishes two live uses of `StationRow` (nearest vs all); as a reload key it is hostile to edits, and `DEFERRED.md` forbids identity matching. LLP 1004 D5 still says teardown; the later carry-by-name rule is the one 1016's `boot_carrying` uses.

**What would change my mind.** A measured reload where hover/`picker` surviving an edit that only restyles the parent is worth identity matching, recorded as a DEFERRED trade.

## P7

**Position.** Concede Sol's `test` whose statements are the eight operations. `expect state password == ""` is a reading of `state` (LLP 1012 §1: `slots` / `derives` / `resources` by declared name), not a ninth op; `expect tree has testId=` is a reading of `tree`. Compiled `has node testId` is the compile-time half, not enough. Drive tests belong beside the app for a multi-file app; in a one-file app they may sit at column 0.

**Reason.** LLP 1006 §2: `contract` is "parsed, not compiled." 0277 G2: "a fraction of its promised depth." 0102 synthesized a debug node because there was no "delivered at least once" marker; LLP 1012 `clock settle` plus `state` listing pending is that marker. A compile-time `has node` would have caught a missing `testId="login-error"` (the bubble's `contract` block; 0099: "you emit the identifiers the choreography will click") and would not have caught a failed login. Castle's error line exists only after `send` fulfils (1016 D5). The frozen `sm-*` choreography is a two-browser script, not a clause in `message-bubble.contract`. `scripts/smoke.mjs` already runs the eight ops on every host (1012). Putting the drive in the language is a grammar over that driver.

**Why `has node` is not enough.** It does not `tap`, `type`, `clock settle`, or read `state`.

**What would change my mind.** A one-file app whose compiled `has node` set, plus the existing smoke, fails a lane that an in-language `test "bad login"` would have caught — and the extra grammar is still cheaper than keeping tests in `smoke.mjs`.

## Inconsistencies

**The rule.** One named-style construct. File-scope `style Name` holds kernel rows; a node applies it by name; later attrs on the node win (r2 P6; 0277 G6). There is no second attribute `token=`.

**What changes in my sample.** `token field` / `token stationLine` become `style field` / `style stationLine`. `input … token=field` becomes `input … style=field`. The merge is lowering; only the word was an alias.

Sol's: the rule is "only a root owns `query`" (1017.002 Reactivity; r2 P4c). `StationsScreen` declaring `query nearby` / `allStations` / `matches` is wrong. Those three stay on the root and pass as props.

## Where 1017 r2 misreads me

1. §5 P5 / §9: "the data crate exporting into the roster with no declaration (Grok)." I wrote: the data crate exports into the same roster table as `stdlib.rs` (name, signature, opcode). The declaration is `plan/tables/format.json` (LLP 1004 D2), not a `.contract` line. Calls are still roster-checked (LLP 1006 §3). Open Q4 asked which table owns them.
2. §5 P7 / §9: "Grok would keep only a compiled `has node testId`." I also asked (1017.001 Q2) "compile-time `has node` only, or also emit an 8-op smoke of `tree` assertions." The constraint was "no ninth agent op," not "tests do not execute."
3. §5 P4c: identity as "declaration plus the use's address plus the enclosing `each` keys" is Sol's sentence, folded as shared. I stored `each`-child slots on the keyed row (LLP 1005 §6) and inlined singleton uses to renamed root slots. I did not key reload on a source address.

## What I concede

- `fn` in Contract, expression-only, for the 1004 "Already Contract" / `coterie-colors` class. The data crate remains the hatch for 0103's list.
- Child `derive` now. The bubble already has it.
- Sol's `test` compiled to the eight operations. `has node` is one `expect tree` line plus a compile check, not the driver.
- `expect state name == value` is a read of `state`.
- `token=` is `style=`. The sample uses `style`.
- `sends` / `refreshes` declared next to `writes` (Sol; 0103's `writes` catches).
- r2's DEFERRED trade for per-instance state: no per-instance `resource` / `mutation`, no `await`.
