# LLP 1018: Durable client state — the store, and a token that never enters the plan

**Type:** RFC
**Status:** Draft (written 2026-08-30, the day it is built; Charlie chose the shape in conversation the same day — "we can build ibex2 and exact2 into whatever is best")
**Systems:** Runner (the `Store`: snapshot in, writes out; the data seam), Contract compiler (bake: no compiled value for a resource that read the store), Web host (`exact_store`; the `store` op over `localStorage`), Apple host (`ibex2::host::Secrets` → the Keychain; the store read before boot and written after commit, never through Swift), Linux host (memory until ibex2 lands there), Agent API (`state` lists store names; agent mode never touches a real store), ibex2 (LLP 0069: the `Secrets` binding and the `secret.keep` grant), Weird Castle (the first consumer: the Castle session token), `host/apple/build.mjs` (macOS signed with the team identity)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Related:** LLP 1016 (asynchronous data settlement — D1 "the runner never does I/O", amended here; D6 grants; §5 named this document: "secure storage of a token across launches (Weird Castle's next ask)"); LLP 1005 §5 (settlement), §6/`Runner::boot_carrying` (the dev reload's carry — the mechanism this reuses across launches), §7 (the seam); LLP 1004 D4 (app data logic lives in a Rust data crate); LLP 1007 §6 (the dev loop carries slots and resources; scroll and focus do not survive); LLP 1012 (the agent API: `state`, `settle`, determinism); ibex LLP 0067 (grants: authority is carried, never inferred), LLP 0068 (`Host`, `endow`, `Bindings`; §2 "synchronous, and why"), LLP 0069 (`Secrets`); Weird Castle `llp/0000` §Authentication and session boundary ("session tokens use Exact's native secure store where the host provides it and browser storage on web"); `rules/DEFERRED.md` §Runtime (a dev reload carries state; no Aquifer data tier; no durable capability grants), §Process (written because it is being built); `rules/RULES.md` §Scope ("the web is the standard")

## Summary

A client that logs in must remember that it did. Weird Castle's first request
(LLP 1016) returns a bearer token — `loginV2 { token }`, sent as
`X-Auth-Token` on every call after — and today that token lives in a mutation
slot that a relaunch or a browser reload forgets. This RFC decides where a
token lives, how it gets there, and what the plan is allowed to know about it:

- **The host owns durable state; the runner still does no I/O — literally.**
  Before boot the host reads the app's granted names into a *snapshot* and
  hands it to the runner; the data crate reads that snapshot synchronously.
  A write does not call the platform: it rides the commit batch as an op
  (`{"op":"store","name":…,"value":…}`), and the host persists it after the
  commit — the shape `command` already has. Reads are synchronous, so the
  **first frame already knows the user is signed in**: the web's
  `localStorage` is synchronous and a web app reads it before its first
  render; an asynchronous secure-store API is the thing every React Native
  app fights with a splash screen, and it would make Exact worse than the
  web at boot.
- **Two tiers, one mechanism.** `secret` — the Keychain on Apple, a `0600`
  file off Apple, `localStorage` on the web — is built now, for the token.
  `plain` — preferences, as persisted slots carried across launches by the
  same op and snapshot — is the next consumer of the mechanism and waits for
  its call site (§5). The web has one backend for both; that is the
  declared deviation.
- **The token never enters the plan.** It is born in the data crate's `parse`
  of `loginV2` and stays below the data seam: the crate stores it, attaches
  the header itself, and the view sees `signedIn` and `username`. `shape
  Session` loses its `token` field; `logout()` takes no argument; the agent's
  `state`, the journal, and a screenshot can never contain a bearer token. No
  Contract syntax changes.
- **ibex2 gets the capability, not the Swift hosts.** `Bindings` gains
  `secrets` beside `fetch`/`fs`/`env`, behind a `secret.keep <name>` grant in
  0067's grammar, with the platform backends where ibex2 already keeps
  platform code (LLP 0069). The Apple host crate reads the snapshot and
  executes `store` ops through it; `glue.js` grows a few lines beside its
  `request` executor.
- **Bake and agent mode see an empty store**, and a resource that read the
  store at bake gets no compiled value — so a plan never carries a
  developer's session, and a smoke never leaves a token in a developer's
  keychain.

## 1. Where this sits

LLP 1016 built the request seam: a data source answers now or hands back a
request, the host executes it, `fulfill` brings the reply to `parse`. Its §5
deferred exactly one thing to "Weird Castle's next ask": a token that
survives a launch. This is that document.

What holds this design to the ground, in the order it decided things:

- **The web is the standard** (RULES §Scope). Its durable client state is
  `localStorage`: synchronous, string key → string value, scoped to the
  origin, read before first render. Whatever the runner's model is, it must
  be that shape; native is allowed to be *stronger* (encrypted, ACL'd to the
  app) but not *different* in semantics or timing.
- **The runner never does I/O** (1016 D1). Kept, literally: the host reads
  before boot and writes after commit; the runner touches a map.
- **The first frame is compiled data** (LLP 1004 D4, 1005 §5) — except where
  the first frame is the device's. A returning user's first frame differs
  from a fresh install's, and no build can know which. D4 below says exactly
  which resources give up their compiled value: the ones that read the store.
- **Capabilities are carried, never ambient** (ibex 0067). The store is a
  binding the host endows from a grant the data crate declares (1016 D6);
  the grant doubles as the list of what the host loads.
- **The data crate is pure and testable** (1004 D4). A test hands the runner
  a snapshot and reads the writes it produced; no keychain, no browser.
- **Determinism for the agent** (LLP 1012). A drive starts from a known store
  — empty — and its writes go nowhere real.

## 2. Decisions

### D1 — The store: snapshot in, writes out

```rust
/// Durable client state as the runner holds it (LLP 1018 D1): the host's
/// snapshot of the app's granted secrets, read before boot; the writes since,
/// which the host persists after each commit.
pub struct Store { … }

impl Store {
    /// The value under `name`, from the snapshot as written since. `None` when
    /// absent — or when `name` is not in the grant, which is the same fact.
    pub fn get(&mut self, name: &str) -> Option<&str>;
    /// Keep `value` under `name`; refused outside the grant.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), StoreError>;
    /// Forget `name`; refused outside the grant.
    pub fn forget(&mut self, name: &str) -> Result<(), StoreError>;
}

/// One write for the host: `value` `None` forgets.
pub struct StoreWrite { pub name: String, pub value: Option<String> }

pub trait DataSource {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError>;           // unchanged: pure, bake's
    fn answer(&mut self, store: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError>;
    fn parse(&mut self, store: &mut Store, source: &str, args: &[Value], outcome: Outcome) -> Result<Value, DataError>;
    fn grants(&self) -> &'static str;                                                     // `secret.keep <name>` lines join `net.fetch`
}
```

`query` is untouched: it is what every in-process source implements and what
bake calls, and it has no store to hand, so nothing existing changes. `answer`
and `parse` — the two methods a source that reaches outside the process
overrides — receive the store as a parameter, the capability-style way: the
authority is in the caller's hand, not in the air. Bake calls `answer` with
an empty store and refuses `Later` as before (D4).

A read is a map lookup. A write updates the map *and* records a
`StoreWrite`; the host takes them (`Runner::take_store_writes`) after the
commit, in order, the way it takes commands. A write inside an action whose
settlement refuses rolls back with the action's slots and commands — nothing
reached the host. A write inside `parse` whose value fails its shape rolls
back the same way. The journal gets one line per write (`store castle.session`
/ `forget castle.session` — the name, never the value); `state` gains
`"store":[…]`, the names present.

Reads and writes are **synchronous** and bounded because they never leave the
runner's memory. That is the amendment to 1016 D1, and it is the web's own
line: I/O the web does synchronously (`localStorage`) the runner does
synchronously — as a snapshot the host filled — and everything else is a
request. The host's own read, before boot, is the platform's synchronous
call (`SecItemCopyMatching` in well under a millisecond on device;
`localStorage.getItem`); a host that ever moves the runner to a worker keeps
the same shape — snapshot in, ops out — which is why the store is not a
callback into the platform from inside `parse`.

### D2 — Two tiers, one mechanism: `secret` now, `plain` next

The op carries the tier — `{"op":"store","tier":"secret",…}` — and the grant
names it. **`secret`** is the platform's credential store: the Keychain on
iOS and macOS, a `0600` file under `$XDG_DATA_HOME` off Apple (libsecret
later), `localStorage` on the web. **`plain`** is the platform's preference
store — `UserDefaults`, an XDG file, `localStorage` — and is *not built here*:
its consumer is a persisted slot (`state station = "sf" persist`, carried
across launches by the same op and snapshot through `Runner::boot_carrying`'s
existing rule: a carried value is taken where it still conforms, else the
initializer), and that lands when Caltrain wants "last station" (§5). Two
tiers are honest because the platforms are: the Keychain is not for bulk
preferences (it survives app deletion, it is slower, it is for small
secrets) and a preference file is not for a token (on macOS every process
running as the user can read `~/Library`). Flattening them would make the
token permanently worse than native on macOS; DEFERRED's "deliberately
worse" list is explicit, and this would not belong on it.

The web's single backend for both is the declared deviation: the web has no
secret store. A word on the exposure: `localStorage` tokens are dangerous on
the web because of third-party script; this page is `glue.js` and a wasm,
with no app JavaScript and no third-party script, so the surface that makes
the pattern scary mostly does not exist here. An `httpOnly` cookie would be
stronger — the browser would hold the secret and the plan would never see
it, D5's rule for free — but it needs a server that sets one for our origin,
and Castle's API returns the token in the body with CORS open (`*`), which
rules out credentialed requests. If Castle ever adds cookie auth, the web
host swaps to it with no change above the seam.

### D3 — The grant is the load list

The data crate declares `secret.keep castle.session` beside `net.fetch
https://api.castle.xyz` in the one `grants()` constant (1016 D6). The runner
parses the complete I/O grant set with the native binding's shared
`exact-grants` grammar (2026-10-02); one invalid line grants nothing. The
Rust-only bake validates the same set, naming the source and line, before
hashing it. Native parsing uses `url`; wasm normalizes origins through a pure
browser URL import, sharing the grammar without shipping another set of URL
tables. Rust logic modules stay importless: their host validates the grants in
the metadata reply. The JS web target parses its complete set with browser URLs and
checks source-scoped fetch and secret access. `Store::get` outside the grant is
`None`, `set`/`forget` outside it are `StoreError::Refused` — the same refusal
on every host before any platform is asked, as `net.fetch` is refused in
`glue.js` and in ibex2 alike. The host reads *exactly the granted names* into
the snapshot (ibex2's `Secrets::names()`; `glue.js` reads every
`exact.secret.*` key and the runner keeps the granted ones), so an
ungranted secret is absent, not refused — `process.env`'s rule (0059.000
§3.8) in one more place.

Names are exact matches, one per line, as `env.read` is. `castle.session`
is the token's; the session record (`{"token":…,"username":…}`) is one value
under it, because a session is one thing to keep and one thing to forget.

### D4 — Bake sees an empty store; a resource that read it has no compiled value

*Amended 2026-09-03 (LLP 1027 D4, ruled): a resource that read the store
now **does** get an `initial` — the empty-store answer, which is a pure
function of the code and never a developer's session, since the bake's
store is empty by construction — and its row says `reader`. For a Rust
source nothing changes: the runner ignores that value and answers at boot
as below. For a source that is not ready at boot (a TypeScript module its
host loads after first pixel), the runner shows the answer it **kept** from
the last launch under `exact.kept.<resource>` beside the app's secrets,
falls back to the compiled placeholder on a fresh install, and asks again at
`data_ready`. The safety property is the same sentence: the dev server never
reads the developer's keychain into a plan.*

`contract::bake` boots the plan against the app's data source with an empty
`Store` that records reads. A resource whose settlement read the store —
`remember()` — gets **no `initial`** in the plan; every other resource is
compiled data as before. At boot the runner's existing rule does the rest:
a resource with no compiled value is queried (`answer`), and `remember()`
answers `Now` from the snapshot the host filled — so the first frame is
right for a returning user, and it is still *compiled* for everything that
does not depend on the device. A `Later` at bake is still the build error
1016 D3 made it.

This is what makes the store safe to bake against on a developer's Mac: the
dev server never reads the developer's keychain into a plan, because the
bake's store is empty by construction, not by discipline.

### D5 — The token never enters the plan

The data crate is the only thing that holds the token. `parse("login")`
stores the session on success and returns `Session { ok, username, error }`
— no `token` field. `answer("logout")` reads the token from the store to
build the request, **forgets it now**, and hands back the `POST`; the
action's `session = none` drops the reply on arrival (1016 §4's assignment
rule), so logout is local and immediate and the server call is
best-effort, as a web app's is. `answer("me")`, when there is a `me`, reads
the header from the store. `remember()` answers the stored session as a
`Session`, `Now`.

Consequences: the agent's `state` dump, the runner's journal, `carry`, and
a screenshot never contain a bearer token; `app.contract` has no storage
vocabulary — a capability is the host crate's business, not the view's
(1016 D6) — and the 1017 restart, which wants a smaller language, gets
nothing added. The alternative that persists the mutation slot itself
(`mutation session … persist`) was rejected: it persists the *reply*, so a
refused login reboots into "Wrong password", and the token sits in the plan.
The alternative of storage as an asynchronous request (a second ticket after
`loginV2`) was rejected by D1: it costs a chained-request extension and a
first frame that cannot know the user.

### D6 — Hosts

- **Web** (`glue.js`, `host/web/src/{abi,batch,host}.rs`). Before `exact_boot`
  the glue reads every `exact.secret.<name>` key of `localStorage` into a
  JSON object and hands it to a new export, `exact_store(len)`, which stashes
  it in the bridge for the next boot; `exact_boot_plan` (the dev reload)
  carries the runner's current store instead (`Carried.store`), which equals
  what the page persisted. The `store` op sets or removes
  `exact.secret.<name>`. In agent mode (`?agent=1`) the glue reads nothing
  and writes nothing: the store is memory, a drive is deterministic, and a
  smoke against real credentials leaves nothing in the profile.
- **Apple** (`host/apple/src/{abi,host,executor}.rs`). At boot the bridge
  parses the data crate's grants, endows `ibex2::host::Bindings` once, reads
  each granted name through `Secrets::get` into the snapshot — on the main
  thread, synchronous, local — and boots the runner with it. After every
  commit `Host` executes the runner's `StoreWrite`s through `Secrets::set`
  and `Secrets::forget` on the same thread (a Keychain write is milliseconds,
  once per login); a write that fails is journaled and the app is otherwise
  unaffected — the next launch will not remember, as a web app that ignores
  `setItem` throwing behaves. Nothing reaches Swift: no ABI change, no
  presenter code. `EXACT_AGENT=1` selects a memory store unless
  `EXACT_STORE=real` says otherwise.
- **Linux** (`host/linux`). A memory store until the Linux host links ibex2
  (ibex OQ2 gates that lane); then ibex2's file store. Declared, not a gap
  anyone trips on: Weird Castle has no Linux build.

### D7 — The platform facts that become requirements

- **macOS: the legacy keychain, and a signed build.** `SecItem*` without
  `kSecUseDataProtectionKeychain` writes the login keychain, whose ACL trusts
  the *creating* app by its code signature's designated requirement. An
  ad-hoc-signed binary's requirement is its `cdhash`, so every rebuild is a
  different app: the keychain prompts ("ExactMac wants to use your
  confidential information…") — and because the host reads the store *before
  boot*, that prompt blocks the first frame. Signed with an Apple Development
  identity, the requirement is `identifier + team certificate`, stable across
  rebuilds, and the app that wrote the item reads it back with no prompt.
  So `host/apple/build.mjs` signs the macOS binary with the identity it
  already locates for iOS devices when the keychain holds one, ad-hoc
  otherwise — no entitlements, no provisioning profile, because the legacy
  keychain needs neither. The data-protection keychain (`kSecUseDataProtectionKeychain`,
  the iOS-style one) is stricter and prompt-free but needs an application
  identifier entitlement, which on macOS means a bundle and a provisioning
  profile: owed when there is a macOS bundle (§5).
- **iOS: the data-protection keychain, `AfterFirstUnlockThisDeviceOnly`.** No
  entitlement for the app's own access group; works on the simulator; a
  live session never rides a backup to another phone. Items survive app
  deletion — a reinstalled app would still be signed in — so a first-launch
  wipe (a marker in Application Support, which *is* deleted with the app)
  is owed with a device test to prove it (§5).
- **Web: origin-scoped**, so the dev loop's `127.0.0.1:8765` and a deployed
  origin hold separate sessions.

## 3. What changes, where

| Where | Change |
|---|---|
| `runner/src/runner.rs` | `Store`, `StoreWrite`, `StoreError`; `DataSource::answer`/`parse` take `&mut Store`; `Runner::boot_with(…, snapshot)`; `Carried.store`; rollback of writes with the action / the reply; `take_store_writes`; `store_names`; journal lines |
| `runner/src/agent.rs` | `state` gains `"store":[names]` |
| `contract/cli/src/lib.rs` | `bake`: an empty recording store; no `initial` for a resource that read it |
| `host/web/src/{abi,batch,host}.rs`, `glue.js` | `exact_store(len)`; the `store` op; the snapshot from `localStorage`; agent mode = memory |
| `host/apple/src/{abi,host,executor}.rs` | `Bindings` endowed once at boot and shared with the executor; the snapshot from `Secrets`; `store` writes executed after commit; `EXACT_AGENT`/`EXACT_STORE` |
| `host/apple/build.mjs` | macOS signed with the Apple Development identity when one exists |
| `host/linux` | a memory store (the snapshot is empty) |
| ibex2 | LLP 0069: `secret.keep` in `grant.rs`/`boundary.rs`; `host::Secrets`; `secrets/{mod,darwin}.rs` and `engine/darwin_keychain.mm`; tests |
| Weird Castle | `shape Session` without `token`; `resource remembered = remember()`; `derive current`; `logout()`; `refresh remembered` in the logout action; the data crate keeps `castle.session`; `GRANTS` gains the line; tests: a snapshot in, writes out, a second boot signed in |
| LLP 1016 D1, 1005 §7, 1007, 1008, 1012 | amended as built: the store beside the request seam |

Sizes, by the code that exists: runner ~150 lines; bake ~15; web host ~60 and
glue ~15; Apple host ~80; ibex2 ~400 with the Keychain shim; Weird Castle ~60.

## 4. Weird Castle

```
shape Session
  ok: bool
  username: string
  error: string

component WeirdCastle
  state screen = "title"
  state who = ""
  state password = ""
  // What the store remembers from an earlier launch: idle at bake (the
  // store is empty there), the kept session at boot.
  resource remembered = remember() as shape Session
  // The reply to the last send: none until one lands.
  mutation session as shape Session
  derive current = match session { case some(s) => s, case none => remembered }
  derive signedIn = current.ok
  derive loginError = current.error
  derive username = current.username
  derive busy = pending(session)

  action submit writes session
    send session = login(who, password)
  action logout writes session, screen, who, password
    send session = logout()      // the crate reads the token itself and forgets it now
    session = none               // the reply is dropped on arrival
    refresh remembered           // re-read: the store no longer holds a session
    screen = "title"
```

`session` overrides `remembered` while it is `some`, which is what makes a
login inside one launch and a remembered one across launches read the same
way; `refresh remembered` after logout is what makes `none` fall back to an
idle session rather than the one the launch remembered.

Data crate: `answer("remember")` → `Now(store.get("castle.session"))` as a
`Session`, idle when absent; `answer("login")` → the form's own refusals
`Now`, else `Later(POST loginV2)`; `parse("login")` → on success
`store.set("castle.session", {"token","username"})`, returns the `Session`
without the token; `answer("logout")` → `store.forget`, then `Later(POST
logout)` with the header from the token it just read, or `Now(idle)` when
nothing was kept. The stand-in (`Castle::stand_in()`, tests and bake) does
the same against the same store, so a test signs in, reads the write, boots
a second runner with that write as its snapshot, and finds `signed-in` on
the first frame.

## 5. Not in this RFC

The `plain` tier and persisted slots (D2 — with Caltrain's "last station");
a `me` query (Weird Castle `llp/0000`'s profile screen; the seam is ready);
the iOS first-launch wipe (D7); the data-protection keychain on macOS (D7 —
with a bundle and a profile); libsecret on Linux and the Linux host's
ibex2 (ibex OQ2); cookies on the web (D2 — Castle's API would have to set
them); a local database or cache of worlds (the "exact1 data island", LLP
0518, research — a later `plain`-tier decision, and the token would not
live in it); encryption of the web's `localStorage` (a key the page holds
protects nothing from the page); a store worker or asynchronous store (D1:
the shape works on a worker as it is); the manifest (0068 §4 — the grant
constant is its seed).

## 6. Process note

No panel: DEFERRED §Process says no refine loops, and the decisions here
follow from rules already accepted (the web is the standard; the runner never
does I/O; capabilities are carried). Charlie chose the shape in conversation
on 2026-08-30 after two alternatives (storage as a request; the persisted
mutation slot) were laid out against it; the document is written because it
is being built the same day (RULES §Scope).
