# Panel: LLP 1023 One URL per app — serving, the envelope, baked bundles, discovery (grok)

- **Family:** grok (xAI): `grok --prompt-file <brief> -m grok-4.6 --reasoning-effort xhigh --disable-web-search --cwd <capsule> --output-format plain`. Reported model not independently verifiable from the CLI; requested `grok-4.6` at `xhigh`.
- **Method:** a panel at Charlie's request, 2026-08-30 — "have a panel discussion about what the best way to do this is." Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind** (neither panelist saw the other), **round 2 mutually visible** (each read the other's round-1 answer in full and wrote only where they differ; one disagreement — the hand-rolled mDNS responder — was named and forced to a final position). Workdir = a read-only capsule, no repository access: the brief, LLP 1023 r1, `CLAUDE.md` + both rules files, exact2 LLPs 1005/1007/1012/1017.000/1018, the dev-loop and agent code (`dev.mjs`, `dev.js`, `serve.mjs`, `app.mjs`, `agent.mjs`, `abi.rs`, `format.json`), and the five exact1 serving documents (app-runtime-and-dev-server, hosting-ssr-ota, LLP 0268, LLP 0109, LLP 0331 — research, never authority; none were ever imported into `llp/research/`).
- **Gates:** LLP 1023 r1 sha256 `c443b8eef9f0f73cee19dd8b3d444dd03f7a999ca268f4d5aa930343d90b25d5`; round-1 brief sha256 `8c5df26167641dc442c4699b9bd8e0d093920162c55499d5bcce559b4dd71e00`; round-2 brief (this panelist's) sha256 `f64368abda3413012d51165a885e3417a58e67b2b1c0cbd7a79fe529e44d7a27`; capsule (evidence files, panel answers excluded) sha256 `6d6ed71ad5130419b4bc05d36a5daa25d85274987ee46d9e2153e27441f40f07`. No capsule file was modified while either panelist was in flight.
- **Runs:** round 1 2026-08-31T00:32:04Z → T00:40:41Z; round 2 T00:43:08Z → T00:49:08Z. Peer: codex `gpt-5.6-sol` at `xhigh` (OpenAI Codex v0.151.0), same capsule, same rounds.
- **Disposition:** folded into LLP 1023 r2 — §11 records which catches held against the live repo and what each round changed. The author re-verified the factual claims before folding (the `FORMAT_DIGEST` derivation, the SSE hello gap, the `dist` prefix form, the `.json` MIME gap all held; the round-1 Weird Castle linkage claim did not survive sol's round-2 correction).

---

## Round 1 — position (verbatim; the CLI's narration lines before the document marker are preserved)

I'll start from the binding rules and the RFC, then walk the cited exact2 specs and code so the position is grounded in this capsule rather than the predecessor's research.The capsule is a closed set of files. Next I’ll read the RFC and the rules that bind it, then the cited specs and host code.The RFC’s claims now need to be checked against the exact2 specs and the host/agent code it says it extends.Next I’ll read the remaining specs and the host/dev-loop code the RFC says it extends.I have enough to take a position: keep the two-rung URL and pointer-card envelope, but Stage 1 is mis-cut, and native code must not go on the wire.# Keep the two-rung URL; keep native code off the wire; cut Stage 1 to the link, not Accept

Panel round 1 (blind). Judged against `rules-RULES.md`, `rules-DEFERRED.md`, the cited exact2 specs and host code, and exact1 as research only. Charlie’s four rulings stand.

---

## 1. Overall position

The RFC’s **thesis is right** for this codebase: one URL names the app; the browser gets `dist/` unchanged; a native client gets a small pointer card and then the plan; the discovery link is the contract and header negotiation is a shortcut; the control plane stays off the LAN; a baked bundle is a `(plan, DataSource)` pair the client already links; there is no update economy, no SSR, no proxied data seam.

That is the correct import from exact1: D2 (one URL) and LLP 0268 (a dumb host always answers HTML; native follows a link). exact2 makes the rest smaller because LLP 1017 P8 already made the plan one self-contained file, and LLP 1005 already made it byte-canonical and digest-gated. Negotiating N per-platform module graphs was exact1’s problem. It is not this repo’s.

I would **build the same shape**, with four load-bearing corrections. None of them is a different product; they are the difference between this RFC shipping as a LAN plan-fetch and accidentally becoming exact1’s update server.

**1. Two payloads is right. Putting the GPU module in the envelope is not.**

D2’s envelope carries `"gpu": { "url": "./deck/night.dylib", … }`. D6 says “the network only ever delivers plans, never native code.” Those cannot both be true. `CLAUDE.md` already treats the GPU module as a separate artifact loaded on demand, not a cargo feature — that does not make it a file the LAN may hand to `dlopen`. iOS will not load an unsigned downloaded dylib; a store binary must not. The web GPU wasm is already fetched as part of the web app (`glue.js` / the wasm), not via a native envelope.

exact1 put executable artifacts in the envelope because the envelope *was* a JS/HBC package (0268’s `startup`, 0109’s module graph). exact2’s plan is not that. Drop `gpu` from the v1 envelope. A `{rebuilt}` that changed the GPU crate is “rebuild the native host,” the same as a data-crate change.

Once `gpu` is gone, **`X-Exact-Platform` has no protocol job**. exact1 needed it because payloads differed per target (0109, `docs/app-runtime-and-dev-server.md` D2). exact2’s plan does not. A custom header a static host cannot honor is exactly the apparatus 0268 spent a five-rung ladder undoing. Telemetry can wait. If the envelope is platform-agnostic, `Vary: Accept` is enough for the optional shortcut, and a static `exact.json` is one file for every client.

**2. The envelope as a pointer card is right. Stage 1 currently inverts 0268.**

The RFC itself: “Negotiation is the dev-server shortcut; the discovery link is the contract.” Then §9 puts LAN + `EXACT_PLAN_URL` + envelope-in-`dist/` in Stage 1, and the link *and* the Accept branch in Stage 2. Without either rung, `EXACT_PLAN_URL` is not the app URL — it is the envelope URL or `/app.plan`. That is a second address. This repo does not keep shims (`rules-RULES.md` §Scope: delete, don’t deprecate). Do not ship a native URL you will have to unteach.

Stage 1 should emit `exact.json` next to `index.html` and a `<link>` that points at it, and the native client should GET `U`, then follow. Accept/`Vary` is Stage 2, an optimization a live server *may* apply — 0268’s actual conclusion.

**3. The registry is right as a type, oversold as Expo Go, and underspecified as Rust.**

D6 is the constraint LLP 1004 D4 already imposed: the data crate is native, on the device, holding the store. A client runs plans for identities it links. That is not Expo Go. exact1’s launcher “never existed” (§1) in part because Expo Go assumes a *shared SDK*. Here the DataSource **is** the app. Weird Castle lives in another repo (`scripts-app.mjs`, `EXACT_APP_DIR`) and cannot be linked into ExactMac from this checkout without a path dependency the RFC does not mention. The honest product is: **one binary, a small closed set of apps, hot-swappable plans for those apps.** The launcher is a project picker plus a URL field, not a universal runtime.

The code change is also not “the macro takes a list.” `host-web-abi.rs` `host!($data:ty, $plan:expr)` (lines 222–257) instantiates one `Bridge<D>` and `exact_boot` / `exact_boot_plan` always do `<$data as Default>::default()`. Several `D: DataSource` types in one binary is an enum (or equivalent erasure) plus a single-entry fast path so Caltrain’s wasm does not pay for it. **Do not put that registry in the web wasm.** Extra data crates in `app.wasm` are download, grants, and parse logic the browser does not need; `dist/` is already one app (`llp-1007-web-host.spec.md` §7, this RFC §7). Native host registry; web `host!` stays one entry.

**4. The bind split is right. The LAN default is right. Interface selection is missing.**

`host-web-dev.mjs:169`, `host-web-serve.mjs:16`, and `scripts-agent.mjs:108` all bind `127.0.0.1`. Keeping the agent there is non-negotiable: `scripts-agent.mjs` is stdio, a Unix socket on iOS (`EXACT_AGENT_SOCKET`), and an ephemeral loopback HTTP server for the web carrier. The RFC is correct that LLP 1018 D4/D5 makes the *plan* session-free by construction, and that this is the same exposure as serving `dist/` to a browser.

What the RFC does not specify: **which `lan-ip` to print**. exact1’s research record (§1) says trusted-LAN selection existed only for physical iPhone, and that is the path Stage 1 claims to unblock. Printing a utun/VPN/bridge address is a silent failure on the phone. Print every non-loopback IPv4; prefer a private-range Wi-Fi address; do not pick.

**Materially different shape I would not take:** skipping the envelope and documenting `U/app.plan` as the native address. That is fewer files this week and the wrong URL on the day the static-host rung lands. Given “delete, don’t deprecate,” the pointer card belongs in the first slice that calls `U` the app URL.

---

## 2. Concerns

### HIGH — Native code on the wire (D2 `gpu` vs D6)

The envelope serves a `.dylib`. D6 forbids native code on the network. iOS code signing forbids loading it. macOS hardened runtime is the same class of problem.

**Resolve:** Remove `gpu` from the v1 envelope. GPU stays a host/bundled artifact. `{rebuilt}` after a GPU-crate edit tells the native client to stop; the human rebuilds the host. Record in §8: no executable native modules over HTTP in this RFC.

### HIGH — No DataSource compatibility gate; `{rebuilt}` will boot the wrong crate

`Runner::boot` refuses a mismatched `kernel_schema_digest` (`llp-1005-plan-and-runner.spec.md` §6). `app_id` (D5) is *identity*, not compatibility. A data-crate edit keeps the same reverse-DNS id and usually the same kernel digest. D3 says: on `{rebuilt}`, re-fetch the envelope, and only drop to the launcher if app id or kernel schema changed. That boots a new plan against an old `DataSource`. Resource names and shapes can disagree; the failure is a data-seam error or a wrong answer, not a typed refusal at the registry.

The `{rebuilt}` event is already a wasm rebuild in `host-web-dev.mjs:121` (`push({ rebuilt: builds })`). On the web that is `location.reload()` (`host-web-dev.js:15`) because a new wasm is a new program. Native has no new program on the wire, by D6.

**Resolve:** Native `{rebuilt}` is terminal for that session: show “rebuild the native host,” keep the last good plan, do not `boot_plan`. A later envelope field (data-crate digest compiled into both the binary and `exact.json`) can make this precise; it is not needed to get Stage 1 right. `app_id` matching is a *second* gate, not this one.

### HIGH — Stage 1 does not actually give the phone a URL, and does not make that URL the app URL

Two holes, one pain:

1. **Discovery.** Stage 1 emits an envelope and teaches `EXACT_PLAN_URL`, but the link and Accept sit in Stage 2. Then `U` for the phone is not `U` for the browser.
2. **Affordance.** The RFC’s own premise is that `--device` passes no environment (cited `host/apple/build.mjs:282` — **that file is not in this capsule; I cannot verify the line**). `EXACT_PLAN_URL` as an env var therefore does not reach the physical iPhone, which is the “sharpest pain” Stage 1 claims to solve. The URL text field is specified as the *launcher’s* second row (D7, Stage 3). The existing dev-menu Reload is “re-fetch now,” which assumes `U` is already known.

**Resolve:** Stage 1 includes (a) `exact.json` + the HTML `<link>` so `U` is the app URL, (b) a **host** URL field on the existing native dev menu — not `apps/launcher/`, not an env var the device never sees. Capsule is silent on how `--device` launches and on whether `devicectl` can pass env; do not bet Stage 1 on it.

### HIGH — “No update economy” is a comment until release hosts refuse network plans

`rules-DEFERRED.md` §Runtime lists Snapback / update economy. §8 of this RFC keeps it out. If a Release ExactMac/iOS binary honors `EXACT_PLAN_URL`, the launcher’s “connect to this URL,” or a canonical production `U`, you have built exact1 `docs/hosting-ssr-ota.md` (“your web host is your update server”) with the envelope as the manifest and the plan as the bundle. 0268’s own security section required signatures and a pinned key before production-capable hosts execute remote artifacts. This RFC has neither, and refuses auth.

**Resolve:** Network plan fetch is a **dev-profile** behavior (`EXACT_DEV`, debug menu, debug Info.plist). Release/embedded-release (0331’s name for the shipped binary) boots the baked plan only. The static `exact.json` beside HTML may exist for *future* work; production native must not consume it in v1. That is the door closing.

### MED — `X-Exact-Platform` plus a single `gpu.url` cannot be one static file

Even if `gpu` stayed, a static host emits one `exact.json`. One `gpu.url` is one platform. The RFC wants the static file to *be* the envelope. A platform-keyed map would still be native-code-on-the-wire. Removing `gpu` and the custom header together is the fix; do not invent `gpu.mac` / `gpu.ios` as a consolation.

### MED — `app_id` in “the header (`plan/tables/format.json`)” is not a real slot yet

`plan-format.json` does not declare the header. `llp-1005-plan-and-runner.spec.md` §2 does: magic `EXPL`, `FORMAT_VERSION` u32, `FORMAT_DIGEST` u64, kernel schema digest u64, compiler identity u64, then pools. `FORMAT_DIGEST` is a hash of the canonical JSON tables, not of the header layout. Adding a reverse-DNS string to the header **does not move `FORMAT_DIGEST` unless `format.json` also changes**. The RFC never mentions `FORMAT_VERSION`.

**Resolve:** Length-prefixed UTF-8 `app_id` after the existing integers, max length bounded (the same MAX_COUNT/RESERVE instinct as 1005), **bump `FORMAT_VERSION`**. Envelope `app.id` is a hint; the plan header wins on disagreement. Display name stays out of the plan, as written.

### MED — LAN address selection, ATS, and the store-binary plist

D8 mentions the macOS firewall prompt and not the interface. D9’s mDNS is Stage 3; Stage 1 is typed URL over HTTP to a printed IP. Open question on `NSAllowsLocalNetworking` is real; the worse miss is **shipping that exception on the store plist**. Capsule has no Info.plist split. The exception belongs on the dev client only.

1018 D7: web store is origin-scoped, so `http://127.0.0.1:8765` and `http://<lan-ip>:8765` are different origins. Native Keychain is bundle-scoped, not origin-scoped. Fetching a plan from `U` must not retarget the store at the origin; it stays the linked app’s store. Say so.

### MED — Envelope is not specified enough to implement without guessing

Missing, and the web has answers for all of them:

- Digest algorithm and encoding (`plan.digest`, asset digests). Do not reuse the truncated 8-byte `FORMAT_DIGEST` shape for payload integrity. SHA-256 hex of the bytes.
- Relative-URL resolution: against the envelope’s **retrieval** URL after redirects (browsers resolve against the document URL).
- Native HTML handling: 0268’s parser-free bounded scan, not a DOM. Native must not execute `dev.js`.
- `Content-Type` for `.json` / `.plan`. `host-web-dev.mjs:131` has no `.json` mapping today.
- Envelope `app.id` vs plan `app_id` precedence.
- What `{seq}` cache-busts (`dev.js` already uses `/app.plan?seq=N`).

**Resolve:** A one-page loader algorithm in the RFC, the length of 0268’s ten-step list, not five rungs.

### MED — Registry sketched as a slogan, not a macro

Stage 3 will stall on: heterogeneous `DataSource` in one `Bridge`; store snapshot per entry (do not `carry` a Castle token into Caltrain); web wasm remaining one-entry; `apps/launcher` as a workspace app whose data crate browses the network (mDNS in *Rust* in the data crate is I/O the runner still must not do — 1016 D1 / 1018: the crate answers from a snapshot the *host* filled). Discovery results as a host-filled snapshot or `Later` request is a real seam; “the data crate’s resources are the discovery results” hand-waves it.

**Resolve:** Before writing Stage 3 code, add an appendix: single-entry `host!` expands as today; multi-entry expands to an enum; web never uses multi-entry; launcher browse is host I/O, store-shaped snapshot in.

### LOW — Reload beacons on `0.0.0.0`

`/__dev/reloaded` and `/__dev/painted` become LAN-reachable log sinks (`host-web-dev.mjs:142–157`). Not a control plane. Acceptable. SSE `/__dev` *must* be on the LAN bind so the phone can subscribe; compile-error text on that stream is the same class of leak as serving the plan.

### LOW — `serve.mjs` on LAN

D8 binds `serve.mjs` too. That process has no compiler and no SSE. Useful for a static demo; not required to unblock the phone. Do not let it drive Stage 1.

### LOW — Capsule-silent citations treated as fact

LLP 1008 §9, LLP 1015 VNC bind-all, `host/apple/build.mjs:282`, “byte-identical 12,580 bytes on five targets,” `EXACT_DEV_PLAN` mtime poll: **not in this capsule**. I am not contradicting them; I am not building on them as verified.

---

## 3. The open questions (§10)

**The flag name.** Keep `--loopback` (and `EXACT_LOOPBACK=1`). One word, no negative, matches Charlie’s off-switch ruling. Implement it as a bind address: default `0.0.0.0`, `--loopback` means `127.0.0.1`. `--no-lan` is a double negative; `--loopback-only` is longer without being clearer. A `--bind <host>` escape is fine if it is sugar, not a second policy.

**Does the envelope live at a fixed name?** **Yes, emit `./exact.json`; the link is still the contract.** The build always writes that file and always points the `<link href>` at it. Clients: if the response is the envelope, use it; if it is HTML, follow the link; tooling may GET `exact.json` without parsing HTML as a convenience, not as a third normative rung. A fixed name with no link teaches clients to guess and breaks the first time href changes. A link with no fixed name makes `curl` worse. Do both; specify two rungs.

Web-standard spelling of the link (RULES: the web is the standard):

```html
<link rel="alternate" type="application/vnd.exact.envelope+json" href="./exact.json">
```

`rel="alternate"` + `type` is HTML’s “another representation of this document.” exact1’s custom `rel="exact:platform-manifest"` existed because the payload zoo needed a private token. A single MIME type does not. `rel="exact:envelope"` as an additional token is harmless; do not *require* a custom rel for the loader to work.

**Where does the launcher’s registry authority live?** **Macro-only.** Agree with the lean. `rules-RULES.md` §Agents: an agent may not add a config file without a human saying so. The linked `DataSource` types *are* the registry. A `launcher.toml` would rot beside the code. Display names live in envelopes / plan-adjacent metadata at serve time, not in a second manifest in the binary.

**iOS ATS.** `NSAllowsLocalNetworking` is the right exception (numeric RFC1918, `.local`). Prove it on a device before calling Stage 1 done. Do not set `NSAllowsArbitraryLoads`. Do not put the exception on the store binary. Print **IPs**, not a `.local` hostname, in Stage 1 — ATS is less surprising against a literal address, and mDNS is Stage 3.

**Does `metrics.mjs` grow a row?** **Yes, diagnostic only.** Envelope → first native frame over the LAN is a number the 100 ms budget row should see. `llp-1007-web-host.spec.md` §7 already treats `metrics.mjs` as non-blocking. Do not add a sixth check (`rules-RULES.md` §Budgets). Do not block Stage 1 on the row existing.

---

## 4. Stage 1 (§9) — what it must not get wrong

Stage 1 is the only slice that ships alone. It has to be a complete “phone opens `U` and runs the live plan” loop, not a pile of parts that only become one URL in Stage 2.

**Must not get wrong**

1. **`U` is the app URL on day one.** Emit `exact.json` from the same path that already writes `dist/` (`build.mjs` and the resident compiler’s `app.plan`). Inject (or bake) the `<link>` the way `dev.mjs` already injects `dev.js` (`host-web-dev.mjs:164`). Native: GET `U`; on HTML, fetch the linked envelope (or `./exact.json` as the emit convention); fetch `plan.url`; `boot` bytes; subscribe to `events` (`/__dev`); on `{seq}`, re-fetch plan and `boot_carrying` as `dev.js` does. No Accept branch yet.

2. **The phone can set `U` without an env var.** Host dev-menu URL field. Until that exists, Stage 1 has not solved the pain it names.

3. **`{seq}` ≠ `{rebuilt}`.** `{seq}` is a contract edit: same binary, new plan, carry state. `{rebuilt}` is a new program the native binary is not: stop, show the error, keep last good. Copying `dev.js`’s `location.reload()` onto native is the bug.

4. **Agent does not move.** No new `/__agent` route on `dev.mjs`. `scripts-agent.mjs` stays loopback / stdio / Unix socket. A smoke under `EXACT_AGENT=1` must not depend on LAN discovery (D7 already says this for the launcher; Stage 1 has no launcher — don’t add a browse “just to try”).

5. **Release binaries do not fetch.** The Stage 1 host change is behind the dev client. Baked `include_bytes!` remains production boot (`llp-1007-web-host.spec.md` §4).

6. **Path prefix stays closed.** `dev.mjs:162` and `serve.mjs:13` already refuse paths outside `dist/`. `scripts-agent.mjs:104` is stricter (`dist + '/'` plus an extension allow-list). Negotiation and `exact.json` must not weaken that. Do not add endpoints.

7. **Bind print is honest.** Both URLs, every plausible LAN IPv4, `--loopback` restores today’s `127.0.0.1`, firewall prompt expected.

**Wrongly in Stage 1 today**

- **Accept / `Vary` / `X-Exact-Platform`.** Shortcut, not the contract. Stage 2.
- **`gpu` in the envelope.** See HIGH.
- **`app_id` in the plan header.** Ruled, but not required to fetch a plan for the one app the binary already is. Stage 2, with the `FORMAT_VERSION` note. Stage 1 may compare `kernelSchema` and refuse before `boot`; that gate already exists in the runner.
- **mDNS, ABI registry, `apps/launcher/`.** Stage 3, as written. Good. Do not pull the launcher forward to paper over the missing URL field.
- **`serve.mjs` LAN bind.** Harmless if envelope emission lives in `build.mjs`; not the critical path.

**Missing from Stage 1**

- The `<link>` (or Stage 1 is not one URL).
- On-device URL entry.
- `{rebuilt}` native policy.
- Digest algorithm, URL resolution, `Content-Type` for `exact.json`.
- LAN interface printing.
- ATS on the **dev** client, device-checked.
- Asset policy, stated: **bundled assets remain the fallback; envelope-listed assets fetch from the serving origin by digest (D4).** D4 is correctly in Stage 1 *if* the envelope exists — a new image in the contract is otherwise a ghost on device. It is also the “sleeper” the RFC names; if it blows the slice, ship bundled-only with that limitation written down, rather than a half-fetch. Do not silently fetch. Hosts currently reading only from disk is an RFC claim; this capsule has no Apple asset loader to confirm.

**Generate `exact.json` in Node, next to the plan, not as a new Rust endpoint.** Hash `app.plan`, write the pointer card when the compiler writes the plan (`host-web-dev.mjs` already learns `{bytes, …}` on stdout). No new process, no new check, no new config file.

Transcribe `1023.000` with Stage 1’s PR, covering **only what landed** (`rules-RULES.md`: a spec without an implementer is not written; `rules-DEFERRED.md` §Process: no speculative specs). Do not pre-write Stages 2–3 into that spec.

---

## 5. The refusals (§8)

**Keep refused, and I would not un-refuse for v1**

- Update economy / production fetch-at-launch.
- Server generation (SSR, route payloads). This RFC is file delivery plus one optional Accept branch. Good.
- Server-proxied DataSource. 1018 D5 (token below the seam, on the device) plus 1004 D4. A LAN “any plan on any client” proxy would also be a new server and a store on the wrong machine.
- Universal declarative DataSource. Language redesign.
- Auth on artifact endpoints in v1. `--loopback` is the embargo switch. Say that in the startup line.
- QR codes (D9). Typed URL + printed IP. QR is apparatus.
- `servedApps` / base-path multiplexing (§7). Two apps, two ports; mDNS later makes them two rows. `scripts-app.mjs` already made multi-repo the unit, not multi-app-one-origin.
- `?platform=` on the app URL. Keep. The URL is the app.

**Refused insufficiently (the door is ajar)**

1. **Update economy.** Close it with “release hosts do not fetch plans,” not with “this document is about the dev loop.” The envelope-on-static-host sentence in §8 is the crack: it is the production-shaped piece, and native loaders that implement D1–D3 will consume it.

2. **Native modules over HTTP.** Not refused; proposed as `gpu`. Add it to §8.

3. **Auth “mechanism would be a bearer token in the printed URL.”** Fine as a trigger note; do not sketch the mechanism in a v1 refusal. That is how OTA grows a second protocol. When a plan is embargo-sensitive, `--loopback` first; a token RFC later.

4. **Expo Go / “any plan whose identity it links.”** D6 is correct as a type rule and undersells the operational truth: identities it *links in this binary*, which for Castle is Castle’s binary. Do not refuse a universal runtime in §8 while the summary sells one.

**Would I un-refuse anything for v1?** No. The URL field in the existing dev menu is not an un-refusal; it is Stage 1 of a ruled launcher, done as host UI instead of a new Contract app. `apps/launcher/` still waits for Stage 3, with Charlie’s ruling.

---

## 6. Recommended next step

Revise this RFC in place (stay Draft). Do not start the launcher, the registry, mDNS, or the Accept branch.

Fold in, then implement **only** the corrected Stage 1:

1. Drop `gpu` (and `X-Exact-Platform` as a protocol requirement) from D2. Add “no native executables on the wire” to §8.
2. Move the HTML `<link>` (and emit of `./exact.json`) into Stage 1; leave Accept/`Vary` in Stage 2.
3. Specify native `{rebuilt}` as session-terminal; `{seq}` as today’s web restart-with-carry.
4. Specify header layout for `app_id` (`FORMAT_VERSION` bump; do not claim `FORMAT_DIGEST` moves unless `plan-format.json` does) — but do not implement it in Stage 1.
5. Specify digest (SHA-256 hex), relative-URL resolution, bounded link scan, and the two-rung loader.
6. Close the update-economy door: network fetch is the dev client.
7. Add the on-device URL field to Stage 1; print all LAN IPv4s; ATS only on the dev plist, device-checked.
8. Sketch the registry macro’s single-entry vs enum expansion as an appendix, explicitly **not** for `host-web-abi.rs`’s wasm `host!`.

Then build that Stage 1 against Caltrain on a physical iPhone: type `U`, see the app, save `app.contract`, see the new plan, save a data-crate file, see “rebuild the host” rather than a poisoned boot. Verify by driving it (`scripts-agent.mjs` remains loopback). Transcribe `1023.000` from that PR.

The RFC already imported the right exact1 conclusion (one URL, HTML first, link is the contract) and the right exact2 facts (the plan is the artifact; the store is empty at bake; the agent is the thing that must never bind all). The mistakes are the ones exact1 already paid for: executable artifacts in the pointer card, a custom platform header with nothing left to select, a Stage 1 that ships the shortcut before the contract, and an OTA door left open by a loader that will work against any static host the moment someone points a release binary at it.


---

## Round 2 — cross-response, mutually visible (verbatim)

I'll re-read the capsule, my round-1 position, and the RFC so I can write only real disagreements against the other panelist's answer.I’ll pull the rest of the cited sources and look for my round-1 write-up so the disagreements stay grounded.I’ll check the capsule citations they lean on—especially the ones that might not hold.**Agreed (one line):** drop `gpu` / `X-Exact-Platform`; web `host!` stays today’s monomorph; network fetch is dev-only; `app_id` is routing not trust; native `{rebuilt}` is terminal; last-good until a complete candidate; Stage 1 needs the HTML link and a typed URL; macro-only registry; `--loopback`; `NSAllowsLocalNetworking` not `NSAllowsArbitraryLoads`; metrics stay diagnostic; tighten `dist` containment; keep the §8 refusals of OTA / SSR / proxied DataSource / QR.

# 1. Disagreements

**Server-side `Accept` is not a Stage 1 requirement.** They put live negotiation in the first slice. That inverts the RFC’s own ladder: the link is the contract, header branching is a shortcut (`llp-1023-one-url-serving.rfc.md:25–31`). Stage 1 is complete when `GET U` returns HTML, the client follows `<link>`, and the phone boots. A client may send `Accept: application/vnd.exact.envelope+json, text/html;q=0.9` from day one; `dev.mjs` need not branch yet. Making that branch a Stage 1 success criterion teaches a live-server URL and leaves the static-host rung untested — the exact1 failure 0268 recorded (`exact1-llp-0268-single-url-static-manifests.rfc.md:773–787`).

**Empty/memory store must not be the `{seq}` default.** Their admission policy is right for a multi-app launcher’s first connect to an unknown URL. It is wrong for Stage 1. That slice is one compiled-in app, and 1007’s loop is restart-with-carry (`llp-1007-web-host.spec.md:210–213`). 1018 already empties bake and agent mode (`llp-1018-durable-client-state.rfc.md:47–50, 183–197`); it does not empty the developer’s Keychain on every LAN reload of the app they just compiled. Native store is bundle-scoped, not origin-scoped (`llp-1018-durable-client-state.rfc.md` D7). Scope empty-store to launcher / unknown-URL admission plus an opt-in, not to the hot path.

**`BundleEntry` is too fat.** DataSource, bundled assets, compiled-in GPU, and a store namespace belong. `permitted host commands / load policy` does not. The capsule’s one named command is `setScheme` (`llp-1005-plan-and-runner.spec.md:177–179`). A command allowlist is new apparatus (`rules-RULES.md:62–67`). Load policy is a shell flag (`dev_load`), not a per-command table.

**`rel="alternate"` + `type` is the link.** They specify `type` and a bounded scan; they leave the RFC’s custom `rel="exact:envelope"`. HTML’s other-representation spelling is `rel="alternate"` plus the MIME type. exact1’s private `rel` existed for a payload zoo this repo no longer has.

# 2. Corrections

- **0268:410–415 is not “well-known derivation.”** Those lines are document-relative vs root-relative URLs inside a manifest. The well-known-path alternative is `exact1-llp-0268-single-url-static-manifests.rfc.md:773–778`. The conclusion (clients must not guess a filename; the link is the contract) still holds; I already wanted emit `./exact.json` as convention only.
- **There is no existing reverse-DNS `app_id` source.** `scripts-app.mjs:19–27` yields `name` (`caltrain`), directories, and crate names. “The one existing source” is a source the RFC still has to name. Putting the header in `plan-format.json` is the right authority; also bump `FORMAT_VERSION`. `FORMAT_DIGEST` is an 8-byte truncated hash of that JSON (`llp-1005-plan-and-runner.spec.md:31, 46–49`), not `sha256:<hex>` of plan bytes — do not conflate the two in the envelope.
- **`imageSource` is not an asset-resolver vocabulary.** `llp-1007-web-host.spec.md:53` maps it to DOM `src`. The unresolved question is envelope `assets[].name` vs bundled files, not `imageSource`.
- **`EXACT_DEV_PLAN` is not in this capsule.** The RFC names it; the only env locator in capsule code is `EXACT_PLAN` (`scripts-agent.mjs:249, 303`). Scheme-dispatch on one locator is still the right apparatus call; do not treat a file-poll implementation as verified.

The `dist` prefix bug is real: `path.startsWith(dist)` accepts a sibling `dist-evil` (`host-web-dev.mjs:161–162`, `host-web-serve.mjs:12–13`); `scripts-agent.mjs:104` is the stricter `dist + '/'` form.

# 3. Concessions

- SSE hello with current `{seq, digest}` is HIGH: `/__dev` writes only a comment (`host-web-dev.mjs:136`), so a native fetch-then-subscribe can miss an edit permanently.
- Stage 1 is bundled assets only; remote D4 waits. I had D4 in the first slice with a fallback — their cut is the one that ships.
- Native `{rebuilt}` is always rebuild/reinstall, even when `app_id` and kernel schema still match (`llp-1007-web-host.spec.md:226–240`).
- ATS is not the whole Apple gate: `NSLocalNetworkUsageDescription` in Stage 1, `_exact._tcp` in `NSBonjourServices` when browsing, plus bounded retry. Capsule-silent, but required on a physical device.
- Metrics: two numbers; hold only hot event→frame against the 100 ms row.
- Envelope `kind`/`version`, `sha256:<lowercase-hex>`, same-origin pointers, ignore-unknown-fields / refuse-unknown-major, and derive registry keys from the baked plan rather than duplicating `app_id` in the macro.
- Print every usable IPv4; state IPv4-only if that is the first bind. Cross-app switch drops slots, resources, requests, timers, store, and asset-cache namespace.

# 4. Forced: mDNS

**Final: system DNS-SD or defer. A ~100-line datagram responder does not stand.**

I was wrong to treat “~100 lines and no dependency” as cheap. A responder that skips PTR/SRV/AAAA, probing, conflict, TTL, and goodbye will fail in the exact places Stage 3 is for (two `dev.mjs` processes, guest Wi-Fi, IPv6), and Apple’s `NWBrowser` will not debug our UDP. That is false economy.

Do not replace it with an npm `multicast-dns` package or a workspace mDNS crate — same cost, prettier name (`rules-RULES.md:62–67`). Advertisement from `dev.mjs` is a system helper already on the machine that is the iPhone story (`dns-sd` / Network.framework). Linux advertisement waits for Avahi, or stays typed-URL. TXT is a hint; port comes from SRV; digest, schema, and `app_id` are taken from the fetched envelope, never from TXT.

Discovery remains Stage 3, after the typed URL works on a physical device. `--loopback` still silences it.

# 5. Five folds, in order

1. **Drop `gpu` and `X-Exact-Platform`.** Pointer card is platform-neutral; `Vary: Accept` only; native GPU is compiled-in. D2 and D6 cannot both be true (`llp-1023-one-url-serving.rfc.md:121–140, 173–180`).
2. **Recut Stage 1 around the phone loop, not the shortcut.** Emit `./exact.json` + HTML `rel="alternate"` link; host-menu typed URL (env vars do not reach `--device`); bundled data/assets/GPU; last-good through every fetch/hash/decode/boot; `{seq}` carries, `{rebuilt}` stops; agent stays on loopback/stdio/socket. Prove it with HTML-then-link, not with Accept branching.
3. **Close network load as a bundle-admission boundary.** Release binaries boot `include_bytes!` only. `app_id` / envelope / DNS-SD never authenticate. Reserve the launcher id for its baked plan. Empty store on unknown-URL admission in a multi-app shell, not on Stage 1 `{seq}`.
4. **Specify the envelope and the reload transaction.** `kind`/`version`; plan SHA-256 hex + byte length + kernel schema (+ format digest, labeled as the 8-byte field); atomic current revision; SSE sends `{seq, digest}` on connect; relative URLs against the final envelope URL; same-origin in v1; `dist + '/'` containment; ATS + local-network privacy on the **dev** plist, device-checked.
5. **Registry is a native `BundleEntry` (DataSource, assets, GPU, store namespace), generated only for multi-app/launcher shells.** Web `host!` stays `Bridge<D>` + `D::default()` (`host-web-abi.rs:20–38, 216–258`). Launcher `openProject(url)` is host I/O, not runner I/O. DNS-SD is a system facility or it waits.
