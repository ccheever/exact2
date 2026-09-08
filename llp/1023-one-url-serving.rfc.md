# LLP 1023: One URL per app — serving, the envelope, baked bundles, and finding a server

**Type:** RFC
**Status:** Draft r2 (r1 written 2026-08-30 after Charlie ruled the four framing questions — LAN by default with an off switch, the launcher is a Contract app, app identity goes in the plan header, 1009 leaves the working set for this; r2 the same day, folding a two-round dual-family panel Charlie convened — grok-4.6 xhigh and codex gpt-5.6-sol xhigh, artifacts `llp/reviews/1023-one-url-serving.{grok,sol}.md`, dispositions in §11)
**Systems:** Plan format (an app identity in the header, declared in `format.json` with a `formatVersion` bump), Runner (no change — `boot_plan` already takes bytes), Web dev loop (`host/web/dev.mjs`: the bind, the envelope, the SSE hello; `host/web/build.mjs`: emitting `exact.json` and the discovery link), Apple host (the URL locator: fetch + subscribe; a typed URL in the existing dev menu), Linux host (same), Contract ABI macro (one compiled-in `DataSource` becomes a `BundleEntry` registry — native multi-app shells only; the web `host!` stays monomorphic), a new `apps/launcher/` (the Expo-Go-like Contract app, Stage 3)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Related:** LLP 1017 P8 (the inliner: a plan is one self-contained file — the fact this whole design leans on); LLP 1018 D4 (bake sees an empty store — a served plan carries no one's session, by construction), D5 (the token stays below the seam, on the device — why the data seam never goes on the wire), D7 (the web store is origin-scoped; the native store is bundle-scoped and does not retarget when a plan arrives from a URL); LLP 1005 (the plan format and its digest gates: `Runner::boot` refuses a mismatched `kernel_schema_digest`, runner/src/runner.rs:374; `FORMAT_DIGEST` is the first 8 bytes of a domain-separated SHA-256 over the canonical `format.json` text, plan/build.rs:210–226 — a *format* identity, not a payload integrity hash); LLP 1007 §6 (the dev loop this rides: SSE `{seq}`, re-fetch, `exact_boot_plan` carrying state; §7: a native Rust change is a new binary); LLP 1012 (the agent API — its carrier stays off the network, always); LLP 1008 §9 + the dev menu (17350d0) (`--device` passes no environment; the phone needs a URL typed into the host, not an env var); LLP 1015 §VNC (the one bind-all precedent, defended for the same LAN reason); LLP 1004 D4 (app data logic lives in a Rust data crate — the constraint §5 states instead of fighting); `rules/NOT-DOING.md` §Runtime (no server generation, no update economy — §8 closes those doors normatively), §Process (written because it is being built, §9); exact1 `docs/app-runtime-and-dev-server.md` D2, `docs/hosting-ssr-ota.md`, LLP 0109, LLP 0268, LLP 0282, LLP 0331, LLP 0003 §905 (research, never authority — none were imported into `llp/research/`; cited from the exact1 checkout)

## Summary

One URL is the whole address of an app. A browser opens it and gets the web
app. A native client opens the same URL and gets the plan. exact1 proved the
model and this RFC imports its conclusion — with the shape exact2's own
decisions have already made much smaller:

- **The artifact is the plan, and it is already perfect for the wire.** One
  self-contained file (LLP 1017 P8), byte-identical across every build target
  (verified: the same hash on wasm32, Linux, macOS, iOS simulator, and iOS
  device — 12,580 bytes for Caltrain), digest-gated at boot, and secret-free
  by construction (LLP 1018 D4). exact1 negotiated among N per-platform
  module graphs; exact2's native payload is *one platform-neutral document*.
  There is no `X-Exact-Platform` header in this design: with nothing
  platform-specific left to select (§11 — the panel removed the GPU field
  that was its last job), `Accept` alone distinguishes the two payloads.
- **The discovery link is the contract; negotiation is the dev-server
  shortcut.** exact1's static tier learned this the honest way (its LLP
  0268): a dumb file host cannot branch on headers, so the first response is
  always the browser HTML, and native clients follow a link in it. exact2
  keeps the ladder two rungs tall: a server *may* answer `Accept:
  application/vnd.exact.envelope+json` directly; on HTML the client follows
  `<link rel="alternate" type="application/vnd.exact.envelope+json">`.
- **A baked bundle is a `BundleEntry` — plan, data crate, assets, GPU,
  store namespace — and a native binary may carry several.** The registry
  keys on the app identity in the plan header. A client runs any plan whose
  identity it links; identity is routing and compatibility data, **never
  trust** (§5). The launcher is entry zero: a Contract app whose data crate
  is fed discovery results by the host.
- **LAN by default; the control plane never; release binaries never fetch.**
  The artifact endpoints bind `0.0.0.0`; `--loopback` turns it off. The
  agent carrier — tap, type, screenshot, state — stays on loopback, stdio,
  and the Unix socket, unconditionally. And network plan loading exists
  only in dev-capable hosts: a release binary boots its baked plan, full
  stop — that is the update-economy door closing in code, not in prose
  (§8).

## 1. What exact1 actually shipped, and what it only planned

The memory of exact1's system is one tier rosier than the code. Both tiers
are worth naming because this RFC keeps one and shortens the other:

- **The dev server truly negotiated.** Native clients sent
  `X-Exact-Platform: ios|mac|windows`; the Vite plugin returned browser HTML
  or a JSON *native envelope* — a manifest of where the code was — with
  `Vary: Accept, X-Exact-Platform` (exact1 `vite-plugin.ts:8142–8178`, LLP
  0109). The envelope was never the code: the client then fetched module
  graphs and Hermes bytecode from `/__exact/*` endpoints. The platform
  header existed because the *payloads differed per target*. exact2's do
  not; the header does not return.
- **The static tier never negotiated, on purpose.** exact1 LLP 0268: a
  static host always answers with the browser HTML, and native clients walk
  a five-rung ladder — decode the body as an envelope, else a manifest, else
  parse the HTML for an inline manifest script, else for
  `<link rel="exact:platform-manifest">`, then fetch and score targets. The
  ladder was the load-bearing contract; header negotiation was documented as
  an optimization a server *may* apply. 0268 also concluded clients must
  never derive a manifest filename by convention — the link says where it
  is (§10 keeps that rule).
- **The Expo-Go part never existed.** mDNS (`_exact-agent._tcp`), QR codes,
  and the "Exact Go" launcher appear only in plan documents; zero code hits.
  What shipped was a manually typed App URL field in the settings overlay,
  loopback binds enforced by the CLI, and trusted-LAN interface selection
  built once, for physical-iPhone runs only.
- **Baked bundles shipped** as the `embedded-release` placement (LLP 0331):
  Hermes bytecode and a baked plan in the app bundle, with
  `docs/hosting-ssr-ota.md` framing the sequel — "your web host is your
  update server" — that §8 keeps out of this document.

None of those serving RFCs were imported into `llp/research/`; the exact1
checkout is the reference.

## 2. What exact2 already has

Every host-independent piece exists and is proven; what is missing is narrow.

- The plan: self-contained, validated on decode, byte-identical across
  targets, refused on `kernel_schema_digest` mismatch. Production boot never
  fetches it — it is `include_bytes!` in the wasm and the native archives.
- Every host boots from bytes (`exact_boot_plan`) and carries state across
  reboots; the web dev loop already moves plans over HTTP (`dev.js` fetches
  `/app.plan?seq=N` on an SSE `{seq}`) and `scripts/agent.mjs` already serves
  `/__plan` to its web carrier.
- The native escape hatches are file-only: `EXACT_PLAN` boots a file,
  `EXACT_DEV_PLAN` polls one — pointed today at the web dev loop's
  `dist/app.plan`, which is why they die at a physical phone: `--device`
  passes no environment (host/apple/build.mjs:282), and the dev menu's
  Reload exists as the workaround.
- Everything binds `127.0.0.1` (host/web/dev.mjs:169, serve.mjs, agent.mjs,
  metrics.mjs) except the Linux VNC server, whose bind-all LLP 1015 defends
  for exactly the reason this RFC does: the LAN use *is* the feature.
- `dist/` is one app at a time; the plan header has a `compiler_identity`
  but no app identity; nothing discovers anything.

The one structural gap: **the app is not just the plan.** It is plan + the
app's Rust `DataSource` crate + optionally a GPU crate + assets. The ABI
macro compiles in exactly one `D: DataSource` (host/web/src/abi.rs:106), so
a client can only boot a plan whose data crate it already links. Everything
in §5 exists to state that constraint instead of fighting it.

## 3. The URL contract

**D1 — one URL, two payloads, two rungs.** For an app URL `U`:

- `GET U`, browser → the web app. Today's `dist/`: `index.html`, `glue.js`,
  `app.wasm`. Nothing changes for browsers.
- A native client sends `Accept: application/vnd.exact.envelope+json,
  text/html;q=0.9` — honestly naming its fallback — from day one. A smart
  server (the dev server, from Stage 2) answers with the envelope and
  `Vary: Accept`. A dumb host answers with HTML.
- On HTML, the client scans (bounded, parser-free — 0268's rule; native
  never executes HTML or `dev.js`) for
  `<link rel="alternate" type="application/vnd.exact.envelope+json"
  href="…">` and fetches that. `rel="alternate"` + a MIME type is HTML's own
  spelling for "another representation of this document"; exact1's private
  `rel` existed for a payload zoo this design no longer has.

There is no platform header and no `?platform=` on the app URL. The URL is
the app, not the target. Same-origin pointers only in v1: everything the
envelope names lives on `U`'s origin.

**D2 — the envelope is a platform-neutral pointer card.**

```json
{
  "exact": 1,
  "app": { "id": "com.exact.weird-castle", "name": "Weird Castle" },
  "plan": { "url": "./app.plan", "sha256": "<lowercase hex>", "bytes": 12580,
            "formatVersion": 2, "kernelSchema": "<u64 hex>" },
  "assets": [ { "name": "assets/weird-castle-mark.png", "url": "./assets/…", "sha256": "…", "bytes": 100 } ],
  "dev": { "epoch": "<32 lowercase hex>", "seq": 41, "generation": "<sha256>",
           "program": "<served program sha256>", "events": "/__dev" }
}
```

- `exact` is the major version: unknown majors are refused; unknown
  *fields* are ignored (additive evolution).
- `plan.sha256` is payload integrity — full SHA-256, lowercase hex, of the
  plan bytes, verified before decode. It is **not** the 8-byte
  `FORMAT_DIGEST`, which is a format identity and travels as
  `plan.formatVersion` + the in-band header. `kernelSchema` lets a client
  refuse before downloading; `Runner::boot` still enforces it after.
- Relative URLs resolve against the envelope's **final response URL**,
  after redirects — the way a browser resolves against the document URL.
- `dev` is the optional live tier; static envelopes omit it. `epoch` identifies
  one server process and `seq` increases within it. `generation` binds the plan
  and complete asset roster, including removals. `program` hashes the actual served app and GPU
  wasm bytes; native connections pin it and require a native rebuild
  if it changes, even when a disconnect hid the transient `rebuilt` event.
  Web GPU wasm remains in the page's own build graph, never this envelope.
- MIME types: the envelope is `application/vnd.exact.envelope+json`; the
  plan `application/vnd.exact.plan`; `dev.mjs`'s type map gains `.json`
  (it has none today, dev.mjs:131).

**Module extension (Codex, 2026-09-07; LLP 1027 D5/D6):** an optional
`module` object contains `native`, `receipt`, and `web` cards, each with
`url`, `bytes`, and `sha256`, naming `app.hbc`, `app.module.json`, and `app.js`.
The pairing receipt binds the plan and both logic forms to app identity,
grants, ABI and bytecode version. Native fetches receipt/HBC; web fetches
receipt/script. Hashes ensure pairing, not publisher authentication. This
slice is admitted development on module-aware web/macOS clients, not Go or
signed delivery; Rust-only clients refuse modules.

**D3 — reload is the dev loop, one network hop longer, and transactional.**
A native dev host given `U` resolves the envelope and subscribes to `dev.events`.
The browser uses the same generation protocol:

- **Hello identifies the whole current generation.** SSE includes `epoch`,
  `seq`, `generation`, `program`, and its immutable `envelope` URL. Before the
  first compile it says `ready:false`. A new process epoch resets ordering;
  retired epochs and completions of superseded fetches cannot commit. Failed
  fetches retain the highest observed revision and retry discovery. Reconnecting
  with the same content does not apply it twice. Every discovered or announced
  revision passes the same program gate; a changed program cancels pending
  host acceptance before the browser reloads.
- **The manifest digest is reproducible.** Hash UTF-8 canonical JSON with object
  keys sorted by UTF-8 bytes and no whitespace or newline:
  `{"assets":[{"bytes":N,"name":"assets/x","sha256":"…"}],"plan":{"bytes":N,"sha256":"…"}}`.
  Assets sort by UTF-8 name. URLs and live ordering fields are excluded. Every
  card has an exact size and SHA-256; clients verify every payload before use.
  When present, canonical `module` is included between `assets` and `plan`,
  with sorted `native`, `receipt`, `web` entries of `{bytes,sha256}`. All three
  count against generation budgets even if a client downloads only its pair.
  The receipt is capped at 1 MiB; either logic artifact at 32 MiB.
  The envelope limit is 64 KiB, each payload 64 MiB, the complete payload set
  256 MiB. Fetches are bounded and obsolete requests are canceled.
- **A candidate replaces the app only when whole.** Clients prepare the plan,
  full asset resolver, fonts, and shader namespace, then accept and commit
  together. Native acceptance covers every session owned by the app (1031).
  Browser font loading leaves the old page running until synchronous host
  acceptance. Omitted names have no embedded or previous-generation fallback.
  Failure preserves the old page, resources, and current generation. An
  unavailable optional GPU does not block core generation acceptance; a
  loaded GPU still validates shaders before accepting the candidate.
- **Payload URLs never change bytes.** `/__dev/generation/<epoch>/<seq>/…`
  serves captured files retained outside rebuilt `dist`, under the app-specific
  `target/dev-generations` cache. Old deck URLs and relative resources survive
  rejected successors, server restarts, and dist replacement. Publication holds
  the filesystem lock, checks the 4 GiB cache quota, writes immutable files, and
  commits the envelope last; incomplete or undeclared files are never served.
  Nothing automatically deletes possibly live namespaces. A full cache refuses
  new publication and names the cache to remove manually after closing its
  pages and native connections. Missing URLs retry current discovery. A new
  wasm suspends old discovery before reload; stopped-compiler output cannot
  republish it. Retained bytes remain readable across that rebuild.
- **Contract edits carry state.** Plan and asset changes use this same full
  replacement operation, without rebuilding the binary for a bundle-only edit.
- **`{rebuilt}` is session-terminal on native.** It means the wasm — the
  *program* — changed (dev.mjs:121); on the web the page reloads into the
  new program. A native binary has no new program on the wire, and a new
  plan against an old data crate is a poisoned boot the digests cannot
  catch (a data-crate edit moves neither `app_id` nor `kernelSchema`; LLP
  1007 §7 already says a native Rust change is a new binary). The client
  keeps the last good app and shows "rebuild the native host." A compiled
  data-crate compatibility digest could later make this precise; it is not
  needed to be safe.

`EXACT_DEV_PLAN`'s file poll stays for the file case; the dev menu's Reload
becomes "re-fetch now."

**D4 — Stage 1 assets are bundled-only.** A plan booted from a URL still
resolves assets from the client's bundle; a plan that references an asset
the bundle lacks is refused with "rebuild the client," not half-drawn.
Fetching envelope-listed, digest-addressed assets from the serving origin
(declared fonts, LLP 1019, ride the same row) is real work in the native
asset path — resolution, caching, integrity — and lands as its own slice
after Stage 1, not silently inside it.

**D10 — one plan is the invariant; platform divergence has a landing
order.** (Added r2, from Charlie's worry that large apps grow per-platform
component versions.) Divergence lands, in order:

1. **The presenter, first.** Look and behavior that differ per platform
   never enter the plan — LLP 1021 is the proof: one set of popover rows
   becomes the browser's top layer, `NSMenu`, and `UIMenu`.
2. **A capability branch, second.** Structural divergence is a `match` in
   the Contract; the plan carries every arm and the runtime selects — the
   web's own model (one document, every breakpoint). The plan is tables,
   not code or assets: the heavy things live outside it and never multiply
   with arms, and unmounted arms do not lay out. Branch on *capability*,
   not platform, where possible — LLP 1020's Linux `{unavailable: true}`
   is `@supports`, not UA-sniffing.
3. **Pruned variants of one source, someday, as an optimization.** The URL
   contract identifies the app, not the bytes: nothing in D1/D2 requires
   every client to receive identical bytes, so if a many-armed plan ever
   measures as too heavy, bake can strip dead arms per target and a smart
   server can hand each client its pruned variant (each with its own
   digest) while a static host serves the fat one. Same source, same
   semantics; no contract change.
4. **Per-platform sources, never.** `.native`/`.mac` forks are the
   four-disagreeing-layers world (`rules/NOT-DOING.md` §Authoring models),
   and this document adds the serving-side reason: they would make the
   plan platform-specific and the URL a lie.

What must hold for this to stay true: the component vocabulary never
forks. `kernel/tables/schema.json` is the one authority; a platform may
*lag* a row's implementation with a declared fallback, never carry its own
version of the row. Divergence in *time* — platforms updating at different
rates — is the update economy's problem, refused in §8, and the header
gates (`kernelSchema`, `formatVersion`, `app_id`) are the refusal
machinery that future document inherits; a plan being validated data means
old-client-meets-new-plan fails closed at decode. One known chafe point:
bake lints the first frame at one phone viewport (390×844, LLP 1017 P1d);
the first real desktop-divergent arm needs a second lint size or a per-arm
story.

## 4. App identity in the plan header

**D5 — the plan says whose it is.** (Ruled: Charlie, 2026-08-30.) The
header gains `app_id`: length-prefixed UTF-8, reverse-DNS, bounded (the
same MAX_COUNT instinct as LLP 1005), the identity the bundle already uses
(`com.exact.weird-castle`).

The mechanics the panel corrected (§11): the header layout is generated
code (plan/build.rs:498), and `FORMAT_DIGEST` hashes the canonical
`format.json` text — a header-only change would move *neither* constant.
So the change is declared where declarations live: `format.json` grows the
header field and bumps `formatVersion`, which moves `FORMAT_VERSION` and
`FORMAT_DIGEST` together through the one authority, and the generated
decoder refuses old plans by version as it already knows how to do
(plan/build.rs:549).

**One code authority for the identity.** Nothing in the repo declares a
reverse-DNS id today (`scripts/app.mjs` derives directory and crate names
only). The leaning (§10): the data crate declares it — `DataSource::app_id()`
— because the data crate is exactly the thing the plan must match. Bake
writes it into the header; a registry entry derives its key from its baked
plan (never a second hand-written id); duplicate ids fail the build. On
disagreement the plan header wins; the envelope's `app.id` is a routing
hint. And identity is *compatibility metadata, never authentication* — §5.

## 5. Baked bundles and the launcher

**D6 — a baked bundle is a `BundleEntry`; the registry is native-only.**
An entry is what a plan needs a host to already have:

- the baked plan and its identity (derived, §4);
- the `DataSource` factory;
- the bundled asset resolver;
- the compiled-in GPU implementation, if any;
- the store namespace.

The ABI macro takes entries; `boot_plan(bytes)` selects by the incoming
plan's `app_id`; no match is a refusal that names the id. One entry
compiles to exactly today's monomorphic code — and **the web `host!` stays
single-entry** (host/web/src/abi.rs:222): extra data crates in `app.wasm`
are download and parse the browser never needs; `dist/` is one app (§7).
The registry is generated only for native shells that bake several apps or
the launcher.

The boundaries that make the registry safe:

- **Identity is not trust.** Any fetched plan can claim a linked app's id
  and thereby select its data crate — a crate that can hold credentials
  below the seam and act on them (LLP 1018 D5 protects the token from the
  *plan*; it does not make an arbitrary plan safe to *drive* the crate).
  The threat model is the trusted dev LAN, stated: a digest delivered by
  the same unauthenticated HTTP peer is integrity, not authenticity.
  Consequences: network loading exists only in dev-capable hosts (§8);
  connecting to a *new* URL in a multi-app shell is a deliberate user
  action; and such an admission defaults to a memory store, with durable
  state a per-connection developer opt-in. The **same-app dev reload keeps
  its own store** — a developer re-fetching the app they compiled is
  `EXACT_DEV_PLAN` with a longer wire, and emptying their keychain on every
  `{seq}` would break LLP 1007's carry for nothing.
- **The launcher's id is reserved.** Its plan boots only from the bundle; a
  network plan claiming the launcher's identity is refused. Launcher
  authority (connect, admit) never arrives over the wire.
- **No cross-app carry.** Switching entries drops slots, resources,
  requests, timers, the store snapshot, and the asset-cache namespace. A
  Castle token never rides into Caltrain.

**D7 — the launcher is a Contract app.** (Ruled: Charlie, 2026-08-30.)
`apps/launcher/`: an `app.contract` whose data crate's resources are the
discovery results and whose one privileged effect is `openProject(url)` —
a narrow launcher-to-shell command, the host swapping which entry runs
(the restart the dev menu already performs). The browse itself is **host
I/O feeding a snapshot** — the runner still does no I/O (LLP 1016 D1, LLP
1018): the host browses, fills the snapshot, the data crate answers from
it. The URL text field — the one affordance exact1 actually shipped — is
its second row, because mDNS fails on guest and corporate networks
constantly. A binary with one baked app boots it directly and reaches the
launcher through the dev menu ("Open project…"); a binary baking several
boots the launcher. Under `EXACT_AGENT=1` discovery stands down the way
the menus arm does — a smoke never browses the office network.

## 6. The bind, and finding a server

**D8 — LAN by default, `--loopback` to turn it off.** (Ruled: Charlie,
2026-08-30.) `dev.mjs` (and `serve.mjs`) bind `0.0.0.0`; `--loopback`
(env: `EXACT_LOOPBACK=1`) restores today's `127.0.0.1` **and silences
advertisement** (D9). The flag name beat `--no-lan` (a double negative);
both panelists endorsed it. What makes the default defensible rather than
merely convenient:

- The plan cannot contain a session — not by discipline, by construction
  (LLP 1018 D4). Serving it to the coffee shop leaks source-shaped data at
  worst, the same exposure as the web app itself. (What D4 does *not*
  license — treating any fetched plan as harmless to execute — is §5's
  boundary, not a reason to stay on loopback.)
- **The control plane is not on the LAN, ever.** The agent carrier (Unix
  socket on iOS, stdio elsewhere, `scripts/agent.mjs`'s server on
  127.0.0.1) does not move; "agent operations unreachable from a second
  machine" is a Stage 1 verification item, not an assumption. The reload
  beacons (`/__dev/reloaded`, `/__dev/painted`) are timing telemetry and
  ride along; the SSE stream must be LAN-visible (the phone subscribes),
  and compile-error text on it is the same class of exposure as the plan.
- **The bind print is honest.** Every usable non-loopback IPv4 is printed
  (IPv4-only stated as the v1 scope), private-range interfaces first; the
  server never silently picks one — exact1 built trusted-interface
  selection for a reason, and a utun/VPN address on the printout is a
  silent failure on the phone. The macOS firewall prompt is expected and
  the startup line says so.
- **Serving hygiene tightens with the exposure.** The static file
  containment check becomes the `dist + '/'` form `scripts/agent.mjs:104`
  already uses (`startsWith(dist)` admits a sibling named `dist-…`,
  dev.mjs:162, serve.mjs:13); only GET/HEAD are served.
- Precedent in-repo: the VNC bind-all, defended in LLP 1015 with the same
  honesty about what it exposes.

**Apple's local-network gate is more than ATS**, and it is dev-plist-only —
the exception never ships on a store binary: `NSAllowsLocalNetworking`
(never `NSAllowsArbitraryLoads`), `NSLocalNetworkUsageDescription` from
Stage 1, `_exact._tcp` in `NSBonjourServices` when browsing lands, and
bounded retry on the first local-network operation while the permission
prompt is unresolved. Verified on a physical device, not the simulator.

**D9 — discovery is the system's DNS-SD, or it waits.** The panel's one
forced disagreement, resolved against the r1 draft (§11): no hand-written
~100-line responder — a responder that skips PTR/SRV/AAAA, probing,
conflict, TTL, and goodbye fails exactly where discovery matters (two dev
servers, guest Wi-Fi, IPv6), and no npm package or workspace crate as a
prettier version of the same apparatus. Advertisement of
`_exact._tcp` (TXT: `app`, `name`, `path`) comes from the system facility
already on the machine that is the iPhone story — `dns-sd` /
Network.framework on macOS; Linux advertisement waits for Avahi or stays
typed-URL. Clients browse with `NWBrowser` on Apple. TXT is a hint: the
port comes from SRV, and digest, schema, and identity come only from the
fetched envelope. The launcher greys a server whose id the client does not
link, with the reason. No QR in v1: it duplicates discovery without adding
trust; it returns only if it ever carries pairing material.

## 7. What one checkout serves

`dist/` is global to the checkout and single-app (host/web/build.mjs:18);
this RFC keeps that. One dev server, one app, one origin — `servedApps` and
base-path multiplexing were exact1 complexity that multi-repo apps do not
need: weird-castle already consumes exact2 by path with `EXACT_APP_DIR`
namespacing its own `target/`, and two apps on one machine are two ports —
mDNS makes them two rows in the launcher. If `dist/` ever moves per-app,
nothing here changes shape.

## 8. Not in this document — and the doors closed in code

- **No update economy, and release binaries never fetch.** Fetching a newer
  plan at production launch — exact1's "your web host is your update
  server" — stays on `rules/NOT-DOING.md` §Runtime. The door closes
  normatively, not rhetorically: **network plan loading is compiled only
  into dev-capable hosts.** A release / embedded binary (exact1 0331's
  `embedded-release`, the shape our store builds take) boots
  `include_bytes!` and neither remembers nor polls any URL. The static
  `exact.json` beside a production web app may exist for future work;
  nothing shipped in v1 consumes it.
- **No native executables over the wire.** The network delivers plans and
  inert assets. No dylib, no wasm-for-native, no GPU module — the r1 draft
  had a `gpu` envelope field; the panel removed it as a contradiction of
  this rule and of iOS code signing, and its removal is what lets the
  envelope be platform-neutral.
- **No identity as trust.** `app_id`, envelope fields, and DNS-SD TXT
  records route and gate compatibility; they never authenticate (§5).
- **No server generation.** The server serves files the build already
  wrote, plus one optional Accept branch. No SSR, no route payloads, no
  per-request work.
- **No server-proxied data seam.** Running the data crate on the server
  would put the store on the wrong machine (LLP 1018 D5) and make a served
  app secretly server-dependent. If it returns, it returns as its own RFC
  with the store split designed first.
- **No universal declarative DataSource.** Reversing LLP 1004 D4 is a
  language redesign, not a serving feature.
- **No auth on the artifact endpoints in v1.** `--loopback` is the embargo
  switch, and the startup line says so. The trigger to revisit is the first
  plan that is itself sensitive; the mechanism is deliberately *not*
  sketched here (a bearer token in a plain-HTTP URL would not survive its
  own threat model — if hostile-network serving is ever wanted, that is
  signatures or pairing, and its own document).

## 9. Staging

Each stage ships alone; none blocks the next's design. Implementer: the
serving lane (this conversation's session under Charlie's sanction,
2026-08-30); the build plan is LLP 1023.000, and `1023.001` is transcribed
from Stage 1's PR covering only what landed.

1. **The phone loop — one URL, the link rung, no server branching.**
   - `build.mjs` emits the static `exact.json` (no `events`) and the
     `rel="alternate"` link into `dist/`'s `index.html`; the resident
     compiler's plan writes keep it fresh (`dev.mjs` already learns
     `{bytes}` on stdout); `dev.mjs` overlays the live envelope, adds the
     SSE `{seq, digest}` hello, binds `0.0.0.0`, grows `--loopback`, the
     `dist + '/'` containment, `.json`/`.plan` MIME types, and the honest
     interface printout.
   - The native dev hosts learn the URL locator — the existing env
     locators accept `http(s)://` (URL ⇒ the envelope flow; no third
     variable) — **and a typed URL field in the existing dev menu**, which
     is what a physical iPhone actually uses (`--device` passes no env).
     Loader: GET `U` with the full Accept header (the server does not
     branch yet — the HTML→link rung is what Stage 1 proves, per 0268's
     lesson), follow the link, fetch/hash/decode/boot atomically, keep
     last-good, subscribe; `{seq}` carries, `{rebuilt}` stops with
     "rebuild the native host." Bundled data, assets, GPU only. Same-app
     store semantics per §5. ATS + usage-description on the dev plist.
   - Verified by driving it, on a physical iPhone and Linux, with the
     browser path unchanged: digest mismatch, truncation, schema mismatch
     refused before download, an edit landing between fetch and subscribe,
     SSE reconnect, `--loopback` from a second machine, path traversal,
     and the agent carrier unreachable over the LAN.
2. **Identity and negotiation.** `app_id` in the header via `format.json`
   + `formatVersion` bump (D5, the one-authority mechanics); the identity
   source (`DataSource::app_id()`, §10); the Accept branch and
   `Vary: Accept` on `dev.mjs` (clients already send the header, so no
   client changes); refusal messages that name apps.
3. **The registry and the launcher.** The `BundleEntry` macro (single-entry
   expands to today's code; enum for multi-entry; never in the web wasm);
   `apps/launcher/` with `openProject(url)` and host-fed discovery
   snapshots; system DNS-SD advertisement and Apple browse; "Open
   project…" in the dev menu; the no-cross-app-carry drop list.

Deferred past all three, each its own slice with its own trigger: remote
digest-addressed assets (D4 — triggered by the first asset edit that hurts
on a device), Linux Avahi, a data-crate compatibility digest.

## 10. Open questions

- **Where does the reverse-DNS identity live?** Leaning
  `DataSource::app_id()` (§4): the crate the plan must match declares the
  name it matches by; bake writes it; registries derive from it; no new
  file, no new syntax. The alternative — a manifest field — adds a file
  agents may not add.
- **The ATS recipe on a real device.** `NSAllowsLocalNetworking` +
  `NSLocalNetworkUsageDescription` + bounded first-op retry is the
  documented shape; Stage 1 owes the measurement, on hardware.
- **Two metrics rows, split.** Cold URL→first-frame over the LAN, and hot
  `{seq}`→first-frame; only the hot number is held against the 100 ms dev
  budget until the cold path has a measured budget of its own. Diagnostic
  (`metrics.mjs`), never a sixth check. (Panel-settled; recorded here until
  the row exists.)

Resolved in r2 (were open in r1): the flag is `--loopback`, both panelists
concurring; the envelope is emitted at `./exact.json` *as convention* while
the link stays the only normative pointer — clients never derive the name
(0268's rule); the registry authority is macro-only with keys derived from
baked plans — no `launcher.toml`; `X-Exact-Platform` is gone rather than
assigned to telemetry.

## 11. Panel fold (r2, 2026-08-30)

Two rounds, two families — grok-4.6 (xAI) and gpt-5.6-sol (OpenAI Codex),
both xhigh, round 1 mutually blind, round 2 cross-visible, one named
disagreement forced to a final position. Verbatim answers and provenance:
`llp/reviews/1023-one-url-serving.{grok,sol}.md`. The author re-verified
factual catches against the live repo before folding. Dispositions:

- **Folded, both families (r1 convergence):** drop the `gpu` envelope field
  and with it `X-Exact-Platform` (the D2/D6 contradiction — native code on
  the wire — was the panel's unanimous HIGH); recut Stage 1 around the
  typed URL on the device and the link rung; native `{rebuilt}` is
  session-terminal; release binaries refuse network plans normatively;
  identity is never trust; web `host!` stays monomorphic; macro-only
  registry authority; `--loopback`; metrics stay diagnostic.
- **Folded, verified against the repo:** the SSE hello race (dev.mjs:136
  sends no current revision — sol HIGH, grok conceded r2); the
  `startsWith(dist)` sibling-prefix fragility (dev.mjs:162, serve.mjs:13;
  agent.mjs:104 already has the strict form — sol MED); the missing `.json`
  MIME type (grok); the header mechanics — `FORMAT_DIGEST` hashes
  `format.json`'s canonical text (plan/build.rs:210–226) and the header
  layout is codegen (:498), so `app_id` is declared in `format.json` with a
  `formatVersion` bump, correcting r1's "the digest moves as it always
  does" (grok MED, sol concurring).
- **Folded, synthesis of a disagreement:** remote-plan store semantics —
  sol's empty-store default scoped by grok's r2 objection to *unknown-URL
  admission in a multi-app shell*; the same-app dev reload keeps its own
  store (LLP 1007 carry, LLP 1018 D7 bundle scoping). Stage 1 cut: grok's
  link-first (server Accept branch in Stage 2) over sol's
  both-in-Stage-1 — the static rung is the contract and gets proven first;
  clients send the full Accept header from day one so Stage 2 costs no
  client change.
- **The forced disagreement:** hand-rolled mDNS. Grok reversed in r2; both
  families' final position — system DNS-SD or defer, no bespoke responder,
  no dependency as a prettier bespoke responder — is D9 as written.
- **Cross-corrections that held:** sol corrected grok's claim that
  weird-castle cannot link from this checkout (path linkage is established;
  what is undesigned is multi-app linkage); grok corrected sol's 0268
  citation (the well-known-path discussion is at :773–778, not :410) and
  sol's `imageSource` framing (LLP 1007 maps it to DOM `src`; the real
  question is envelope asset names vs bundled files, now D4).
- **Not folded:** sol r1's `permitted host commands / load policy` row in
  `BundleEntry` — grok's r2 objection held: the capsule's one named command
  is `setScheme`, and a command allowlist is new apparatus with no consumer
  (`rules/RULES.md` §Agents). Its `load policy` survives only as the
  dev-capable/release split in §8. Also not folded: assigning
  `X-Exact-Platform` a telemetry-only role (both r1s flirted with it;
  carrying a spoofable header with no job is how vocabularies grow).

## 12. Delivered — Stage 1, 2026-08-30

Landed the same day as the fold, in three commits (server 4b3ace7, Apple
0e4cb07, Linux ab8baa2), transcribed as LLP 1023.001 with the build plan
at 1023.000. As-built deviations from §9, each carried to QUEUE where
still owed: the Linux loader is one-shot per run over ibex2's transport —
the host already linked it for the request seam, so the planned std-only
HTTP client was never written, and the live SSE half waits for a Linux
display; the native pre-download `kernelSchema` compare waits on the
client exporting its own digest (the runner's boot gate still refuses
after download); the physical-iPhone typed-URL run waits on an unlocked
phone (the ATS plist and signing are proven on the built bundle); the dev
bin gained `--once` so `build.mjs` writes `dist/app.plan` and `dist/` is a
complete static deploy. Verification included one unplanned live test: the
peer session's weird-castle `dist/` swap fed a foreign plan to the
caltrain binary through a bare python static server, which booted it with
a data-seam error — the recorded demonstration that Stage 2's `app_id`
gate is load-bearing, not ceremony.

**Stage 2 landed the same evening** (96ed314, after Charlie ratified the
`DataSource::app_id()` leaning): the header identity via `format.json`'s
new `header` declaration and the `formatVersion` 1→2 bump, bake writing
the id, `Runner::boot`'s `AppMismatch` gate (unnamed-matches-anything
keeps fixtures bootable), `app.id` in the envelope, and the `Accept`
branch with `Vary` on `dev.mjs` — zero client changes, as designed. The
Stage 1 incident replayed as a refusal naming both apps. Deviation:
weird-castle's own declaration waits on its in-flight data file; its
plans bake unnamed and still boot everywhere until it lands (QUEUE). The
remaining §9 stage is the third: the `BundleEntry` registry, the
launcher, system DNS-SD.
