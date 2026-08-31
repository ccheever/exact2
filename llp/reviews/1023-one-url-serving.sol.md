# Panel: LLP 1023 One URL per app — serving, the envelope, baked bundles, discovery (sol)

- **Family:** OpenAI: `codex exec -m gpt-5.6-sol -c model_reasoning_effort=xhigh -s read-only --skip-git-repo-check -` (OpenAI Codex v0.151.0), prompt on stdin, cwd = the read-only capsule; no repository access. Requested `gpt-5.6-sol` at `xhigh` (the author picked xhigh over ultra for symmetry with the grok panelist).
- **Method:** a panel at Charlie's request, 2026-08-30 — "have a panel discussion about what the best way to do this is." Not a refine loop: no verdict binds, no approval given or withheld. Two rounds: **round 1 blind**, **round 2 mutually visible** (each read the other's round-1 answer in full and wrote only where they differ; the hand-rolled-mDNS disagreement was named and forced to a final position). Capsule contents: see the grok artifact — identical capsule, identical gates.
- **Gates:** LLP 1023 r1 sha256 `c443b8eef9f0f73cee19dd8b3d444dd03f7a999ca268f4d5aa930343d90b25d5`; round-1 brief sha256 `8c5df26167641dc442c4699b9bd8e0d093920162c55499d5bcce559b4dd71e00`; round-2 brief (this panelist's) sha256 `7c188649df4f197b5684bd1df3a1697d73b11855d77e70bcaaae9ffcc6065acc`; capsule sha256 `6d6ed71ad5130419b4bc05d36a5daa25d85274987ee46d9e2153e27441f40f07`. No capsule file was modified while either panelist was in flight.
- **Runs:** round 1 2026-08-31T00:32:04Z → T00:42:30Z; round 2 T00:43:08Z → T00:47:42Z. Peer: grok `grok-4.6` at `xhigh`, same capsule, same rounds.
- **Disposition:** folded into LLP 1023 r2 — §11 records which catches held. The author re-verified before folding: the SSE no-hello race (dev.mjs:136), the `startsWith(dist)` prefix form (dev.mjs:162, serve.mjs:13 — fragile as a boundary; live traversal limited by URL dot-segment normalization), and the header/`FORMAT_DIGEST` correction (plan/build.rs:210-226, :498) all held; the round-2 correction of grok's Weird Castle linkage claim held; the ATS additions (NSLocalNetworkUsageDescription, NSBonjourServices, bounded first-op retry) are Apple-documentation facts adopted as stated requirements pending the device check the RFC already owed.

---

## Round 1 — position (verbatim)

# LLP 1023 round-1 position: Keep one URL; make native loading a bundle-admission boundary

## 1. Overall position

The RFC has the right product shape: one app URL, browser HTML by default, a small native pointer document, static-host discovery through HTML, an explicit separation between artifact delivery and the agent control plane, and a Contract-authored launcher.

I would materially change three implementation choices.

First, negotiate only on `Accept` in v1. If every native platform receives the same byte-identical plan, the native envelope should also be platform-neutral. Remove `X-Exact-Platform` from representation selection and from `Vary`; retain it only as optional logging metadata, if at all. Its stated remaining purpose is selecting a downloadable GPU artifact ([llp-1023-one-url-serving.rfc.md:135–140](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:135>)), but that conflicts with the stronger and correct rule that the network delivers plans, never native code ([same file:173–180](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:173>)).

Second, the registry should contain complete native bundle capabilities, not merely `(app_id, plan, DataSource)`:

```text
BundleEntry
  baked plan / app identity
  DataSource factory
  bundled asset resolver
  compiled-in optional GPU implementation
  store namespace
  permitted host commands / load policy
```

Selection belongs in the native shell or launcher composition layer. The runner should continue to execute one validated plan against one concrete data source. The current ABI is deliberately monomorphic—`Bridge<D>` owns `Host<D>`, and the macro constructs `D::default()` at boot ([host-web-abi.rs:20–38](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/host-web-abi.rs:20>), [host-web-abi.rs:216–258](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/host-web-abi.rs:216>)). Do not make every web ABI pay for multi-app indirection. Generate the registry only for shells that include the launcher or several apps; the one-app form should remain today’s generic code.

Third, the envelope should be a strictly specified, platform-neutral pointer card:

- `kind` and major `version`;
- app identity and display name;
- plan URL, SHA-256, byte length, plan-format digest, and kernel-schema digest;
- optional integrity-addressed asset mappings;
- a dev-only events URL and current revision;
- no native dylib URL.

The overall flow should be:

```text
URL
  → Accept negotiation, or HTML discovery link
  → validate envelope and URL policy
  → fetch, hash, and fully decode plan
  → validated plan.app_id selects BundleEntry
  → instantiate one host with that entry’s data/assets/GPU
```

The launcher remains a Contract app, but it needs one explicit, narrow launcher-to-shell effect such as `openProject(url)`. Its baked plan should be privileged and non-replaceable: a network plan must not gain launcher authority merely by claiming the launcher’s `app_id`.

The bind split is correct. Artifact endpoints may be on the LAN; agent operations remain on pipes, sockets, or loopback. But “the plan contains no stored token” does not imply “any plan is harmless to execute,” and the RFC should stop conflating those properties.

I would defer mDNS until the typed-URL path works on a physical device. When added, use a proper DNS-SD implementation, not the proposed hand-written ~100-line datagram responder.

## 2. Concerns

- **HIGH — The proposed GPU field violates the registry’s security model.** The envelope offers a platform-specific dylib while D6 says the client runs only code it already links. A downloaded dylib is also not a portable iOS mechanism.  
  **Resolution:** Remove native GPU URLs. Native GPU support is selected from the matched `BundleEntry`; web GPU assets continue through the ordinary web build.

- **HIGH — `app_id` is compatibility metadata, not authentication.** Any fetched plan can claim a linked app’s ID and thereby obtain that app’s data crate. A plan can initiate resource and mutation activity through the data seam; the token staying below that seam does not make those capabilities inert ([llp-1005-plan-and-runner.spec.md:125–145](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1005-plan-and-runner.spec.md:125>)). LLP 1018 proves that the developer’s stored token is not serialized into the plan—not that all compiled resource data is non-sensitive or that an arbitrary plan may safely drive the linked crate ([llp-1018-durable-client-state.rfc.md:183–209](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1018-durable-client-state.rfc.md:183>)).  
  **Resolution:** Remote-plan loading is debug/development-only, always requires deliberate user action, uses an empty or memory store by default, and never treats envelope or mDNS identity as trust. Reserve the launcher ID for its baked plan. Production automatic loading remains refused.

- **HIGH — The reload protocol lacks a coherent revision transaction.** A client can fetch a plan and subscribe after the corresponding SSE event, missing an edit permanently. The envelope’s digest also becomes stale whenever `app.plan` changes. On `{rebuilt}`, the RFC may continue if app ID and kernel schema still match, but a Rust data-crate change can be incompatible without changing either. LLP 1007 already says native Rust changes require a new binary ([llp-1007-web-host.spec.md:226–240](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1007-web-host.spec.md:226>)).  
  **Resolution:** The dev envelope must represent the current plan atomically; the event stream must send the current `{seq, digest}` immediately on connection; clients compare it with the booted digest. A native `{rebuilt}` always keeps the last good app and asks for rebuild/reinstall. Candidate fetch, hash, decode, and boot must complete before replacing the running host.

- **HIGH — Stage 1 does not actually solve physical-iPhone entry.** The RFC correctly says device launches receive no environment, but Stage 1 supplies only `EXACT_PLAN_URL`; the typed launcher and “Open project…” arrive in Stage 3 ([llp-1023-one-url-serving.rfc.md:85–89](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:85>), [same file:276–287](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:276>)).  
  **Resolution:** Move the dev menu’s typed “Open project…” action into Stage 1. Environment variables remain useful for macOS, simulator, Linux, and automation.

- **MED — App identity’s serialization and source are unspecified.** The present plan header is fixed and contains no string ([llp-1005-plan-and-runner.spec.md:46–52](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1005-plan-and-runner.spec.md:46>)); the format JSON declares tables but not a header schema ([plan-format.json:1–6](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/plan-format.json:1>)). The RFC says bake reads an app manifest, but the capsule’s app resolver exposes only app name, directories, and derived crate names ([scripts-app.mjs:19–27](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/scripts-app.mjs:19>)).  
  **Resolution:** Put the header layout in the single generated format authority; specify encoding, length/grammar limits, and the one existing source of `app_id`. Fully decode and validate the plan before using its ID. Do not duplicate the ID in the macro entry list.

- **MED — The wire contract is not yet implementable interoperably.** Digest algorithm/encoding, JSON limits, redirect behavior, URL base, same-origin policy, MIME types, unknown-field handling, and HTML link parsing are unspecified.  
  **Resolution:** Use an explicit `kind/version`; `sha256:<lowercase-hex>`; bounded envelope and plan sizes; HTTP(S) only; relative URLs resolved against the final envelope response URL; same-origin pointers in v1; required digest verification; unknown additive fields ignored and unknown major versions refused. Native `Accept` should honestly include the fallback: `application/vnd.exact.envelope+json, text/html;q=0.9`. If the response is identical across native hosts, emit only `Vary: Accept`.

- **MED — Asset provenance is larger and less defined than Stage 1 admits.** It is unclear which `imageSource` values are logical bundle names, remote URLs, or runtime data, and a fetched plan currently still runs against app code and assets compiled into the client. Calling the plan “the entire app artifact” while assets and GPU remain external obscures this boundary.  
  **Resolution:** Define asset resolution as part of `BundleEntry`. Keep Stage 1 on the current app’s bundled assets; refuse a newly referenced missing asset with “rebuild the client.” Add remote integrity-addressed assets only after the resolver semantics are specified.

- **MED — LAN exposure turns existing file-serving looseness into a security boundary.** `dev.mjs` and `serve.mjs` use a string-prefix containment check that can accept a sibling whose name begins with `dist` ([host-web-dev.mjs:159–166](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/host-web-dev.mjs:159>), [host-web-serve.mjs:11–15](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/host-web-serve.mjs:11>)). Binding `0.0.0.0` also says nothing about multiple interfaces, VPNs, or IPv6.  
  **Resolution:** Reuse the stricter `dist + separator` containment form already present in `scripts-agent.mjs`; allow only intended methods; enumerate and print all usable interface URLs; state IPv4-only explicitly if that is the first implementation; verify from a second machine that agent operations remain unreachable.

- **MED — A bespoke “minimal mDNS” implementation is false economy.** Correct DNS-SD involves PTR/SRV/TXT/address records, probing/conflict handling, TTL/goodbye behavior, multiple interfaces, and IPv4/IPv6. TXT data is unauthenticated and can only be a discovery hint.  
  **Resolution:** Use a system DNS-SD facility or a vetted implementation; otherwise defer discovery. Advertise only stable hints and the app path; obtain the port from SRV, then fetch and revalidate the envelope. Never accept digest, schema, or app ID from TXT as authoritative.

## 3. Positions on the §10 open questions

1. **Flag name:** Use `--loopback`. It is short and describes the resulting bind. Its documented semantics should include suppressing DNS-SD, not merely changing the HTTP address.

2. **Fixed envelope name:** Emit `exact.json` as the generated-build convention, but make only the HTML link normative. Clients must not derive the filename. Document-relative links survive base paths; the predecessor’s static-host research identified fixed well-known derivation as the thing that breaks at deep paths ([exact1-llp-0268-single-url-static-manifests.rfc.md:410–415](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/exact1-llp-0268-single-url-static-manifests.rfc.md:410>)). Specify the exact build-emitted link form, including `type`, so native clients need only a bounded scanner rather than a general HTML parser.

3. **Registry authority:** Macro-only; no `launcher.toml`. The macro should list full bundle entries but derive each key from its baked plan, failing on duplicate IDs. A second config file would violate both the single-authority goal and the repository’s apparatus rule ([rules-RULES.md:62–67](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/rules-RULES.md:62>)).

4. **ATS:** `NSAllowsLocalNetworking` is the narrow ATS declaration; do not use `NSAllowsArbitraryLoads`. It is not the whole Apple requirement. Stage 1 also needs `NSLocalNetworkUsageDescription`; Stage 3 Bonjour browsing needs `_exact._tcp` in `NSBonjourServices`. Apple also warns that the first local-network operation can initially fail while permission is unresolved, so the client needs `waitsForConnectivity` or bounded retry behavior. See Apple’s [NSAllowsLocalNetworking](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowslocalnetworking) and [local-network privacy technote](https://developer.apple.com/documentation/Technotes/tn3179-understanding-local-network-privacy). Verify on a physical device; simulator success is insufficient.

5. **Metrics row:** Yes, in the existing diagnostic script, never as a sixth check. Report two numbers separately: cold URL/envelope-to-first-frame and hot event-received-to-first-frame. Hold only the latter against the existing 100 ms dev-restart budget until cold LAN behavior has a measured budget. `metrics.mjs` is already diagnostic and non-blocking ([llp-1007-web-host.spec.md:253–262](</private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1007-web-host.spec.md:253>)).

## 4. Stage 1

Stage 1 should be thinner but more complete vertically:

- Extend the existing `EXACT_DEV_PLAN` locator to accept either a file path or an HTTP(S) app URL; do not add a third overlapping environment variable unless a real ambiguity requires it.
- Add LAN/`--loopback`, `Accept` negotiation, and the HTML link in this stage. As currently staged, the client fetches an envelope before either negotiation or the link exists.
- Add the physical-device “Open project…” typed URL action now.
- Serve a minimal, versioned envelope with plan digest, bytes, schema, current revision, and events.
- Boot only the app already compiled into the client, using its existing bundled data, assets, and GPU implementation.
- Subscribe with an immediate current-revision event and retain the last good running host through every fetch, hash, decode, and boot failure.
- Treat any `{rebuilt}` as “rebuild/reinstall native client required.”
- Exercise the path on an actual iPhone and Linux, while confirming the browser path is unchanged.

Stage 1 must test digest mismatch, truncation/oversize, schema mismatch before plan download, an edit between fetch and SSE subscription, reconnect after a dropped SSE connection, `--loopback` from a second device, path traversal, and the inability to reach the agent carrier over LAN. It should not yet contain remote asset fetching, app identity, the multi-app registry, the launcher, or mDNS.

## 5. The refusals

Keep the refusals of update economy, SSR/server generation, a server-proxied data seam, and a universal declarative `DataSource`.

Strengthen these doors:

- **No remote native code:** remove the dylib URL, explicitly including GPU implementations in this refusal.
- **No production auto-fetch:** release single-app binaries neither remember nor poll a canonical URL. Remote loading exists only in an explicitly development-capable shell.
- **No identity-as-trust:** `app_id`, envelope fields, and DNS-SD TXT records are routing and compatibility data only.
- **No cross-app carry:** changing app ID drops slots, resources, requests, timers, store snapshot, and asset cache namespace.
- **No half-loaded revision:** retain the last good host until a complete candidate stands.
- **No custom mDNS stack:** DNS-SD uses a maintained facility or waits.

I would not add accounts or authentication merely to hide web-public artifact GETs. I would un-refuse a minimal remote-plan admission policy: explicit user action, development-only execution, and a memory/empty store by default, with real durable state requiring a deliberate developer opt-in. If the author instead wants automatic production loading or authenticated execution on hostile LANs, signatures or cryptographic pairing become prerequisites; a bearer token exposed in a plain-HTTP URL is not sufficient. QR remains refused unless it later carries such pairing material rather than duplicating discovery.

## 6. Recommended next step

Revise the RFC before writing `1023.000`:

1. Replace the data-only registry with the full `BundleEntry` boundary.
2. Remove platform-dependent GPU delivery and reduce negotiation to `Accept`.
3. Specify the envelope and reload state machine completely, including transactionality and `{rebuilt}`.
4. Recut Stage 1 around manual URL entry on a physical iPhone, existing bundled capabilities, and the two-rung URL contract.
5. Leave DNS-SD and remote asset provenance for separately proven follow-on slices.

Then build that Stage 1 against Caltrain and demonstrate, from one URL, unchanged browser boot plus cable-free iPhone plan reload while the agent carrier remains unreachable from the LAN.


---

## Round 2 — cross-response, mutually visible (verbatim)

Agreements unchanged, one line: remove `gpu` and `X-Exact-Platform`; move the discovery link and on-device URL entry into Stage 1; keep the agent loopback-only.

## Disagreements and corrections

- **Macro-only is not a complete `app_id` authority.** Bake must write the identity before the host macro sees the plan, but the capsule has no reverse-DNS app manifest—`resolveApp` derives only a directory/crate name ([scripts-app.mjs:19](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/scripts-app.mjs:19)). The fold needs one code authority shared by bake and registry; the macro should verify, not independently declare, it.

- **A single build-emitted `exact.json` cannot express live versus static serving.** The RFC says `events` is absent on static hosts ([LLP 1023:138](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:138)), while `serve.mjs` serves `dist/` verbatim ([host-web-serve.mjs:11](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/host-web-serve.mjs:11)). Static `exact.json` should omit `events`; `dev.mjs` must overlay that same path—or inject a dev-specific linked envelope—with SSE metadata.

- **Move network asset provenance out of Stage 1.** It changes native asset resolution, caching, integrity, and fonts, and the RFC itself calls it the sleeper work item ([LLP 1023:153](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:153)). Stage 1 should support existing bundled assets and state that asset edits still require rebuilding the host.

- **Dev-only gating does not fully answer unauthenticated plan integrity.** A plan controls resource and mutation calls, while the linked data crate can hold credentials below the seam ([LLP 1018:35](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1018-durable-client-state.rfc.md:35)). The fold must explicitly declare trusted-LAN execution and use a separate dev store namespace, or add authenticated source integrity. A digest delivered by the same unauthenticated HTTP peer is not authentication.

- **The Weird Castle path-dependency claim is false.** The capsule explicitly says its external workspace consumes exact2 by path and all scripts resolve through that workspace ([scripts-app.mjs:2](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/scripts-app.mjs:2), [CLAUDE.md:27](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/CLAUDE.md:27)). Multi-app linkage remains undesigned, but external linkage itself is established.

- **ATS was overstated.** The capsule says `NSAllowsLocalNetworking` is “probably right” and requires a device check ([LLP 1023:300](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:300)); the panel cannot promote that to a verified answer.

## Concessions

- `{rebuilt}` should be session-terminal until a compiled DataSource compatibility digest exists.
- Release binaries must normatively refuse network plans, not merely rely on the RFC’s “dev-loop” framing.
- The registry belongs only in native multi-app hosts; web wasm remains single-entry.
- Adding `app_id` requires an explicit header layout and `FORMAT_VERSION` bump; the current JSON declares tables, not that header ([LLP 1005:48](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1005-plan-and-runner.spec.md:48)).

## Forced disagreement: mDNS

Final position: **do not ship the hand-written responder.** “~100 lines” is unsupported by implementation or verification, and exact1’s discovery never shipped ([LLP 1023:223](/private/tmp/claude-501/-Users-ccheever-projects-exact2/ea40ceb6-70af-4ddc-8921-1b73718ecdab/scratchpad/panel-1023/capsule/llp-1023-one-url-serving.rfc.md:223)). Use system DNS-SD/Bonjour/Avahi abstractions, including for Linux browsing, or defer discovery and retain typed URLs. A bespoke responder/listener is protocol apparatus disguised as a line-count win.

## Ranked fold changes

1. Make Stage 1 genuinely one-URL: linked envelope plus on-device URL entry, with live/static envelope behavior defined.
2. Close the execution boundary: dev-only network loading, separate dev store/trusted-LAN statement, and terminal `{rebuilt}`.
3. Remove `gpu`, native executable delivery, and `X-Exact-Platform`.
4. Define one `app_id` authority, header/version encoding, DataSource compatibility, and native-only registry mechanics.
5. Defer network assets and mDNS; later use system DNS-SD rather than custom packet code.
