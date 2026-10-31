# Review of LLP 1154 (exact2 and Snapback move independently): codex gpt-6.1-sol

## r1 (2026-10-31)

**Provenance.** Family: OpenAI, codex `gpt-6.1-sol`, reasoning effort `ultra` (both in the codex run header), session `01a19162-e508-7462-a01f-4cbd32371974`, run by Claude (Fable 5.1) for Charlie Cheever, read-only (`codex exec -s read-only --ephemeral`) in the round-1 review capsule: a `git archive` of `e6183cf993` (worktree `exact2-1154`, on origin/main `b510e2a837`), capsule hash `e3b0c44298fc1c14…`. Target: r1, sha256 `05acd14e55f6…`. Brief sha256 `1e16cf47eabaae7d…`. Blind: the capsule held no other review and no note of this loop. Not redacted. The text below is the reviewer's final message, unedited; dispositions follow in the r2 section of the target.

---

NOT READY — The ownership decision is sound, but the package does not yet fit the stated seam, and the cutover would leave broken callers and uncheckable success criteria.

## Direction

Snapback should own and publish its Exact integration, including the client protocol, device binaries, driver and tooling. A package-owned Rust shim over an opaque JSON C ABI is a reasonable boundary: Exact need not know Snapback’s symbols, private Rust types or release number. The problem is the claim that the remaining platform work is mostly linking and Apple composition. LLP 1133.001 defines more than is built, and the existing Snapback integration uses a different dispatch and storage path. Those gaps need explicit decisions in this RFC. The dependency order—prove the seam, publish a working consumer package, then remove the old integration—is right.

## MATERIAL concerns

1. **The proposed package declaration is not bindable through the existing seam.**

   **Where:** D2, RFC lines 110–130; §3 Builds.

   **Evidence:** D2 specifies a root `exact.json` of `kind: "implementation"` without identifying its interface or per-platform implementations. LLP 1133.001 D1 requires an implementation to name `implements` and `platform` (`llp/1133.001-modules-anyone-can-write.rfc.md:92`). The actual Rust builder enumerates interface packages, then selects implementations of those interfaces ([packages.rs:156](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3075bb39-112b-4b60-a056-edc166a1f89f/scratchpad/r1/capsule/contract/cli/src/native/packages.rs:156)); its implementation matching requires `implements` and the requested target at line 201. TypeScript’s module discovery likewise accepts only root declarations of `kind: "interface"` (`js/bake/src/typescript.mjs:1096`).

   Consequently, shipping the described implementation package and importing `Snapback` does not establish the binding the RFC assumes.

   **What would resolve it:** Specify the interface and implementation arrangement, including how the driver reaches its declared native call and how the plan records that use. A single npm package can carry the interface, driver and endorsed implementations. Make the proof fixture exercise that arrangement, rather than only a separately authored Contract call.

2. **Composing a Rust file does not supply the adapter’s authority, storage or transport integration.**

   **Where:** D2’s native-module bullet; D3, especially lines 143–145; §3 item 2.

   **Evidence:** Today Harmony explicitly installs Snapback in the TypeScript data module’s app-owned native slot (`apps/harmony/app/apple/src/lib.rs:10`, `:23`, `:34`; Linux equivalents at `apps/harmony/app/linux/src/main.rs:11`, `:24`, `:35`). The driver calls that slot directly (`snapback4/ts/snapback.ts:236`).

   The current adapter requires baked identity and grants, configured host directories, grant admission and capability-based SQLite path resolution ([snapback4/src/lib.rs:55](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3075bb39-112b-4b60-a056-edc166a1f89f/scratchpad/r1/capsule/snapback4/src/lib.rs:55), `:78`, `:151`, `:176`). Linux’s package trait and context provide a different interface: `later`, replies, notifications and session services, without the adapter’s storage configuration or grant inputs (`host/linux/src/package.rs:29`, `:145`, `:158`).

   D3 also conflates two layers. `Module::call` returns JSON synchronously (`snapback4/src/lib.rs:103`); `host_request` is a separate helper for Rust callers’ HTTP exchanges (`:207`). Implementing `Package` does not itself give a shim an `Answer::Later` transport.

   **What would resolve it:** Choose the runtime route and add its missing capabilities to §3. State how a package obtains an admitted physical partition path and session lifecycle, and whether HTTP remains driven by TypeScript or requires a host continuation API. The fixture should open storage through that route and refuse an ungranted path. Leaving the Apple trait’s spelling to implementation is reasonable; leaving these semantics implicit is not.

3. **Android needs dispatch and shim composition, not merely a linked archive.**

   **Where:** Summary lines 27–29; §3 items 1–2; §4 item 2; §5 step 2; §8 question 3.

   **Evidence:** LLP 1133.001 explicitly says Android composes no Rust packages and uses Kotlin implementations (`llp/1133.001-modules-anyone-can-write.rfc.md:462`). The code enforces this: `rust_packages` only binds Linux and Windows, while an Android triple selects the uncomposed Android branch (`contract/cli/src/native/packages.rs:83`, `:133`). Android package discovery requires Kotlin sources and a class (`js/bake/src/typescript.mjs:1247`).

   The RFC supplies a Rust shim and adds Apple composition, but supplies neither Android Rust composition nor a Kotlin/JNI bridge. A successfully linked Android library would therefore still lack the package call route. Also, §5 requires six native targets before deletion, while §8 permits Android to follow.

   **What would resolve it:** Include an Android dispatch/composition decision and an Android proof, or consistently defer Android and make its absence visible under D6. Reconcile the publication gate with that choice.

4. **The deletion list leaves a broken Harmony lane and other Snapback-specific callers.**

   **Where:** D1, D5; §3 Deletes; §5 step 3.

   **Evidence:** The `harmony` lane invokes `scripts/memory-backend-test.mjs` (`scripts/async.mjs:163`). That helper unconditionally executes the device builder being deleted ([memory-backend-test.mjs:110](/private/tmp/claude-501/-Users-ccheever-projects-exact2/3075bb39-112b-4b60-a056-edc166a1f89f/scratchpad/r1/capsule/scripts/memory-backend-test.mjs:110)). It installs dependencies only when `node_modules/snapback4/package.json` is absent (`:106`), so it does not establish D5’s promise to test Harmony’s current lock.

   Other omitted callers include:

   - `packages/background/test/harmony.mjs:60`, which also runs the deleted builder.
   - `scripts/agent-launch.test.mjs:236`, which imports the deleted preflight module and is included in the async glue list (`scripts/async.mjs:118`).
   - The Snapback-specific `backend` verb (`scripts/exact.mjs:1307`), plus app CLI registrations and preflight wiring (`scripts/app-cli.mjs:85`, `:256`).
   - Snapback schema, persona and change-feed knowledge beyond `fresh` in `scripts/agent-test.mjs:211`, `:232`, `:615`.

   **What would resolve it:** Include these consumers in the cutover. Put Snapback-specific server supervision and installation in Harmony or the published package, install from the frozen Harmony lock before using either dependency, and remove or transfer the remaining special hooks and their tests in the same change.

5. **The publication prerequisite excludes replacements that the deletion requires.**

   **Where:** §4 items 4–5; §5 steps 2–3; §3 Deletes.

   **Evidence:** Step 2 requires only §4 items 1–3. It does not require `init --exact`, coordinated `upgrade`, the replacement guide or its retest. Step 3 nevertheless deletes Exact’s scaffold commands, guide and qualification apparatus. D2 additionally promises package-owned preflight before testing, but the transition never demonstrates how that preflight is invoked once Exact’s hooks disappear.

   The existing scaffold performs generation, mount wiring, identity checks and device construction (`game/new.mjs:574`, `:583`, `:596`, `:616`). The existing retest exercises installed consumers and updates claims only after success (`snapback4/README.md:1575`; `scripts/retest-snapback-readme.mjs:74`).

   **What would resolve it:** Gate deletion on a published, installed-consumer workflow that includes initialization, upgrade, preflight, documentation and replacement tests—not just binaries. Snapback must adopt those obligations before Exact relies on them. No cross-repository atomic commit is necessary, and the dependency order does not force exactly three commits.

6. **The measures cannot currently establish the claimed independence.**

   **Where:** D1; §6 M1, M3 and M5.

   **Evidence:** M1 matches this RFC itself: its command contains `"snapback4"`, and its prose contains the forbidden repository URL and version pins (`llp/1154-exact2-and-snapback-move-independently.rfc.md:36`, `:39`, `:260`). Historical governing documents also contain such facts (`llp/1133.001-modules-anyone-can-write.rfc.md:407`; `llp/1086-building-with-exact2.rfc.md:475`). D1’s literal prohibition on version numbers anywhere outside `apps/` therefore conflicts with retaining this design and its sources.

   M3 can pass after editing two dependency versions even if the new packages are never installed, built or used. M5’s “answers” can also be satisfied by a substitute unless it explicitly establishes native execution; the tally proof distinguishes bound implementation results from substitute results (`packages/fixtures/tally/test/linux.mjs:27`, `:37`).

   **What would resolve it:** Scope the prohibition to active dependencies, executable integration and maintained instructions, with explicit treatment of historical documents. Make M3 reinstall, rebuild and run the upgraded app, checking actual device identity. Make M5 assert a library-specific result under native binding. M2 and M4 are useful clean-environment measures once the missing lane work is included.

## Minor concerns

1. **Prebuilt-library feasibility needs qualification, not a different architecture.** The hard part is the complete link closure: deployment/API floors, Linux libc baseline, position-independent code where required, system libraries, and symbol coexistence. Exact already bundles SQLite (`vendor/ibex/crates/ibex2-sqlite/Cargo.toml:12`), while Snapback’s device depends on `rusqlite` (`snapback4/Cargo.lock:1013`). An independently built archive could introduce another SQLite implementation or Rust runtime objects. That is a risk to test, not evidence that the design is infeasible. Apple’s app crate is an rlib (`apps/harmony/app/apple/Cargo.toml:9`), and an existing final-link path builds a staticlib (`host/apple/build.mjs:1421`); a Cargo check alone proves neither link.

2. **The wasm asset addition is necessary and correctly identified.** Publish and capture the matching wasm-bindgen glue with the wasm, including their identity and rebuild inputs. Today the builder generates that glue (`snapback4/web/build.mjs:150`), and the driver imports it (`snapback4/ts/web.ts:18`). The current web builder copies app-owned assets (`host/web-js/build.mjs:805`); package JavaScript capture is not already arbitrary asset capture (`js/bake/src/typescript.mjs:505`, `:584`).

3. **Add a retained-data upgrade exercise to the proof.** Fresh tests do not show that the replacement reopens an existing partition with its identity and queued work intact. The relevant invariants already exist in `snapback4/client/src/partition.rs:6`, `:17` and `snapback4/src/lib.rs:180`. This is verification work, not a demand for a compatibility branch.

4. **Correct the prose counts and ambiguity.** “Twenty files” is nineteen (`scripts/check-snapback-readme.mjs:8`; `snapback4/readme-tested.json:34`). “Eight releases behind” includes an unreleased main-branch version in the RFC’s own account. “No shim” at RFC line 253 should say no transitional compatibility shim, since D3 deliberately requires a package shim. The summary’s “two items” should agree with §3’s four-item build list.

5. **Trim the diary and clarify apparatus approval.** The dated release procession and commit-by-commit account at RFC lines 46–52 exceed what explains the decision; `AGENTS.md:53` and `CLAUDE.md:53` prohibit that style. The fixture is appropriate first-consumer evidence, and no seventh blocking check is needed. Any added fixture scripts/configuration still fall under `rules/RULES.md:63`; Status should distinguish the quoted ownership request from explicit approval of added apparatus, as LLP 1133.001 itself does at line 522.

## Claims checked

| Claim | True/false/unverifiable | Evidence |
|---|---|---|
| Three upstream crates are pinned to one revision and exact version. | True | `snapback4/Cargo.toml:15`–`:17`; matching lock entries at `snapback4/Cargo.lock:995`, `:1013`, `:1025`. |
| Those upstream crates are `publish = false` and unavailable from a registry. | Unverifiable here | The local adapter’s `publish = false` at `snapback4/Cargo.toml:27` does not establish the upstream crates’ publication policy. |
| Root and Harmony install `snapback4@0.4.33`. | True | `package.json:13`; `apps/harmony/package.json:6`. |
| Harmony has its own Cargo workspace because its adapter brings the private source. | True | `apps/harmony/app/Cargo.toml:1`; `Cargo.toml:32`; dependency at `apps/harmony/app/apple/Cargo.toml:19`. The documented history agrees: `llp/1036.001-apps-outside-the-repo.rfc.md:117`; `llp/1153-a-website-in-exact.rfc.md:269`. |
| Harmony mounts the checkout’s TypeScript driver. | True | `apps/harmony/app/app.json:38`. |
| The receipt hashes twenty files. | False | Nineteen entries: `snapback4/readme-tested.json:34`; authoritative list at `scripts/check-snapback-readme.mjs:8`. |
| The retest executes four complete apps and qualifies their claims. | True, with an omitted fifth exercise | It also runs the auth reload recipe: `scripts/retest-snapback-readme.mjs:14`; receipt examples at `snapback4/readme-tested.json:7`. |
| The adapter depends directly on upstream Rust APIs. | True | `snapback4/src/lib.rs:28`, `:188`; `snapback4/client/src/lib.rs:21`; `snapback4/client/src/partition.rs:17`; `snapback4/web/src/lib.rs:12`, `:79`. |
| The protocol client drives a device through one JSON-call trait. | True | `snapback4/client/src/client.rs:21`; re-export at `snapback4/client/src/lib.rs:17`. |
| Current preflight permits compatible device/server version skew. | True | Stamp parsing and floor/capability comparison at `snapback4/web/device-build.mjs:14`, `:53`. |
| An app cannot run “Snapback newer than exact2’s pin.” | False as written | Newer compatible **servers** are allowed by that preflight. The checkout pins the compiled **device**. The summary must distinguish them. |
| Linux and Windows already compose package Rust sources. | True, within stated limits | `contract/cli/src/native/packages.rs:83`; `host/linux/src/app.rs:628`; limits at `llp/1133.001-modules-anyone-can-write.rfc.md:442`. |
| Linking is the remaining Android work. | False | Android Rust composition is excluded at `contract/cli/src/native/packages.rs:83`, `:133`; Kotlin discovery at `js/bake/src/typescript.mjs:1247`. |
| The current Harmony lane reliably installs what Harmony’s lock specifies. | False | Installation is conditional on package absence, followed by an unconditional source-device build: `scripts/memory-backend-test.mjs:106`. |
| D3’s package shim uses `Answer::Later` “as today.” | Misleading | Current `Module::call` returns JSON; Rust HTTP helpers are separate: `snapback4/src/lib.rs:103`, `:207`. The package trait uses `Reply`: `host/linux/src/package.rs:38`. |
| Snapback’s C header, published wasm exports, compatibility floor, Exact checker, release qualification, initializer and upgrader behave as described. | Unverifiable here | These are upstream assertions in RFC §1 and §4; the local adapter cannot establish their upstream implementations. |
| The six repin dates, subsequent release list and cited commit’s changes are accurate. | Unverifiable from the supplied files | The current pins are established above; the historical sequence is not. |

The most consequential upstream dependency is **publication of the new device-plus-client ABI**, including resumable client dispatch and engine identity. An existing four-function device ABI does not establish that richer contract. Next are the preservation of partition/persistence behavior and the claimed floor-and-capability semantics. Initializer, upgrader and installed-consumer qualification claims become prerequisites for deleting Exact’s tooling.

## Strongest alternative

Keep the same Snapback-owned npm package and JSON binary boundary, but publish **Swift and Kotlin/JNI package bridges for Apple and Android, with a Rust bridge for Linux**, using the host-language package seams already present. Apple discovery currently consumes Swift implementations (`js/bake/src/typescript.mjs:1208`), Android consumes Kotlin (`:1247`), and Linux consumes Rust (`contract/cli/src/native/packages.rs:83`).

This retains Snapback’s ownership and keeps the protocol in one binary implementation while potentially avoiding new Apple and Android Rust-package runtimes. It costs a few small bridges in different languages. The RFC should compare that cost with building and maintaining those new runtimes. Rejecting a generic Exact-owned C ABI dispatcher does not reject this alternative: each bridge would still own its symbols and implement the existing package contract.

NOT READY — The ownership decision is sound, but the package does not yet fit the stated seam, and the cutover would leave broken callers and uncheckable success criteria.
## r2 (2026-10-31, delta round)

**Provenance.** Family: OpenAI, codex `gpt-6.1-sol`, reasoning effort `ultra` (both in the codex run header), session `01a19172-fcaf-7641-8955-98f31ee88fd5`, run by Claude (Fable 5.1) for Charlie Cheever, read-only in the round-2 capsule: a `git archive` of `f69e5b18fc` with `llp/reviews/1154-*` removed, capsule hash `e3b0c44298fc1c14…`. Target: r2, sha256 `19e4f405772c…`. Brief sha256 `3b9b2f669672199f…` (it carries the r1→r2 diff and asks for IN-DELTA/PRE-EXISTING classification). Blind as in r1. Not redacted. The text below is the reviewer's final message, unedited.

---

NOT READY — r2 fixes the native seam, but the transition omits existing consumers and M1 cannot pass as written.

## Direction

The ownership decision and revised native boundary are sound. `NativeModule` is the slot Harmony uses; generating its factory and putting grant admission behind `NativeStorage` can remove the shim’s dependencies on `ibex2` and `exact-runner`. A C ABI implemented by a separate Rust dynamic library is feasible in principle, with packaging and symbol isolation to prove through the fixture. The four build items are apparatus, and Status records approval covering them; I found no additional approval requirement for those items under `rules/RULES.md:63–66`. The principal external dependency is Snapback supplying the **wider module ABI**, including client dispatch and durable partition semantics—not merely its existing device ABI. That cannot be verified here. The supplied SHA-256 matches; this review used static, read-only inspection.

## MATERIAL concerns

1. **IN-DELTA — Step 1 changes a trait with more consumers than §3 acknowledges.**

   **Where:** RFC §3 item 2, lines 277–282; §5 step 1, lines 356–358.

   “The trait’s one consumer is the adapter” is false. Excord implements it on Windows (`apps/excord/windows/src/main.rs:20–34`) and Android (`apps/excord/android/src/lib.rs:28–43`). Exact’s tests implement it too (`js/tests/it/storage.rs:106–124`, `js/tests/it/native_later.rs:18`). Harmony’s existing wrappers implement the current three-directory signature (`apps/harmony/app/apple/src/lib.rs:10–18`, `apps/harmony/app/linux/src/main.rs:11–19`).

   Replacing `configure_storage(data, cache, temporary)` with `configure_storage(NativeStorage)` therefore requires updating these implementations. As written, step 1 cannot both change the trait and leave its existing callers untouched.

   **Resolution:** Include every affected implementation in step 1’s atomic change, including adapting Harmony’s existing wrappers while retaining its paths, pin and adapter. Correct the single-consumer claim. No compatibility branch is needed.

2. **PRE-EXISTING — The deletion list leaves dependencies on files and exports it removes.**

   **Where:** RFC §3 Deletes, lines 296–325; §5 step 3, lines 361–363. Deleting the whole adapter and scaffolding exports was already in r1.

   A root Cargo test contains a compile-time dependency on the deleted tree:

   ```rust
   include_str!("../../../../snapback4/online/projection.ts")
   ```

   This is at `contract/cli/tests/it/typescript.rs:594–598`. Deleting `snapback4/` without addressing it makes the test target fail to compile. Likewise, `game/new-app.test.mjs:13` imports `wireSnapback`, which §3 removes. `host/web-js/build.test.mjs:352–385` tests the Snapback-specific declaration transformation that §3 deletes.

   These are concrete surviving consumers, so §R’s claim that every caller joins the deletion is inaccurate. M4 cannot pass by performing the listed deletions alone.

   **Resolution:** Add the corresponding test removals or ownership transfers to step 3. Preserve any general Exact coverage using Exact-owned fixtures; do not retain copied Snapback implementation code merely to satisfy those tests.

3. **IN-DELTA — M1 rejects unrelated dependencies and therefore cannot establish its stated pass condition.**

   **Where:** RFC §6 M1, line 372.

   Its final grep matches **every** `0.4.*` version outside `apps/` and `llp/`. The root lock contains unrelated dependencies such as Chrono `0.4.45` (`Cargo.lock:720–722`) and getrandom `0.4.3` (`Cargo.lock:2445–2447`). These remain after removing every Snapback pin.

   Consequently, “all three empty” requires unrelated dependency changes that D1 does not call for. Conversely, the numeric branch would miss a bare Snapback version after it leaves the `0.4` series.

   **Resolution:** Scope dependency checks to Snapback package/source records and prose checks to Snapback references. Remove the global version-number pattern.

## Minor concerns

1. **IN-DELTA — Preserve the native-package selection rules explicitly.** D2 adds a declaration outside the existing module kinds (RFC lines 184–188). State that this native slot still requires a direct dependency, exact pin, installed/locked agreement, and refusal of competing slot providers. Those requirements belong to LLP 1133.001 D4/D9 (`llp/1133.001-modules-anyone-can-write.rfc.md:259–267,399–413`). Also identify how D6’s “optional” marking applies to a package with no interface declaration. This is implementation specification, not an objection to using the slot.

2. **IN-DELTA — Dynamic-library packaging needs more than copying the file.** D3, lines 230–238, should acknowledge final Apple linker inputs, framework install names/search paths, signing, and Linux runtime lookup. Apple’s current Rust archive reaches the Swift link through `host/apple/Package.swift:55–60`; GPU signing/capture is a separate path (`host/apple/build.mjs:901–930`). Have the fixture launch the packaged app without build-tree library paths and verify that bundled SQLite symbols remain private. Separate libraries make isolation possible; the fixture should demonstrate it.

3. **IN-DELTA — Preserve identity and storage lifetime across the new ABI.** D3/§4 list `open(real_path, origin, viewer, backend)` but omit how the library receives `appId`. Existing partition identity includes it (`snapback4/client/src/partition.rs:11–12`; `snapback4/src/lib.rs:165`). Also retain the directory authority while SQLite remains open: today `OpenPartition` retains `AppDirectories` (`snapback4/src/lib.rs:31–35`), and physical SQLite paths require stable ancestry (`vendor/ibex/crates/ibex2/src/stdlib/app_fs_unix.rs:385–406`). These should be explicit obligations of `NativeStorage` and §4’s ABI ask.

4. **IN-DELTA — Complete the measurement instructions.** M3 should name the lock-updating upgrade step before its frozen install and identify the engine-version observable. Current client status contains connection/outbox state, not an engine version (`snapback4/client/src/client.rs:773–793`). M5 exercises native libraries but does not exercise §3 item 3’s package web asset. M6 should cover both native SQLite and the web’s retained-change representation (`snapback4/web/src/lib.rs:120–130`) before deletion.

5. **PRE-EXISTING — Specify where app tests invoke the package’s preflight.** D4 promises preflight before a test run, while D2 now describes running it before the package’s own tests. Those are different entry points. The existing Exact hook runs on web drives (`scripts/agent-test.mjs:611`), and the current preflight reads a wasm stamp (`snapback4/web/device-build.mjs:53–66`). §4 should explicitly cover application test invocation and the native library’s actual identity.

6. **IN-DELTA — `Package` does have a synchronous method.** RFC §R line 16 and §7’s rejection should say that these hosts do not provide synchronous dispatch. The trait contains `Package::call`, defaulting to `NotSupportedError` (`host/linux/src/package.rs:39–42`); LLP 1133.001 records the dispatch limitation at lines 459–461. The corrected wording still supports choosing `NativeModule`.

7. **IN-DELTA — Remove the revision ledger from the design.** §R, lines 12–36, is review history and attribution rather than governing design. `rules/DEFERRED.md:1704–1708` explicitly refuses revision ledgers. Keep the resulting decisions and necessary rationale; shorten Status accordingly. This also follows `AGENTS.md:51–55`.

8. **PRE-EXISTING — Tighten two promises.** “Above the floor” should be “at or above”: equality passes the implementation (`snapback4/web/device-build.mjs:63–66`). “Exact keeps [the seams] stable” at RFC lines 122–123 also promises more than `rules/DEFERRED.md:1717–1719`, which explicitly disclaims public API stability. Independence here means ownership of the adapter and release artifacts, not an immutable Exact API.

## Claims checked

| Claim | True/false/unverifiable | Evidence |
|---|---|---|
| The supplied file is r2 with the specified hash | True | SHA-256 matched `19e4f405…b830560`. |
| The adapter pins three upstream crates to one revision and exact version | True | `snapback4/Cargo.toml:15–17`. |
| Root and Harmony npm dependencies name `0.4.33` | True | `package.json:13`; `apps/harmony/package.json:6`. |
| The receipt hashes nineteen files and records five examples | True | `snapback4/readme-tested.json:7–53`; `scripts/check-snapback-readme.mjs:8–12,29–32`. |
| Harmony attaches Snapback through the data module’s native slot | True | `apps/harmony/app/apple/src/lib.rs:9–35`; `apps/harmony/app/linux/src/main.rs:10–36`. |
| The host configures native storage before using the instance | True | `js/src/lib.rs:663–686`; trait contract at `js/src/native.rs:45–56`. |
| The adapter is the trait’s only consumer | False | Excord implementations at `apps/excord/windows/src/main.rs:20` and `apps/excord/android/src/lib.rs:28`, plus Exact tests. |
| `Package` has no synchronous call | False as written | It has `call`; its default refuses. `host/linux/src/package.rs:39–42`. |
| The adapter performs grant admission and resolves a physical SQLite path | True | `snapback4/src/lib.rs:151–179`. |
| The adapter’s Rust `DataSource` transport has no app consumer here | True in the inspected source | External adapter uses are Harmony’s native wrappers; transport helpers are defined at `snapback4/src/lib.rs:207–252`. |
| Linux/Windows package composition currently uses `#[path]`, not a generated Cargo manifest | True | `contract/cli/src/native/packages.rs:83–116`; governing qualification at LLP 1133.001:442–458. |
| Existing Apple builds provide framework/dylib packaging precedents | True, with distinct paths | `host/apple/build.mjs:200–207,683–692,901–930`; `host/apple/modules.mjs:2–13`. |
| Exact bundles SQLite | True | `vendor/ibex/crates/ibex2-sqlite/Cargo.toml:12`. |
| §3 identifies every consumer affected by deletion | False | Compile-time include at `contract/cli/tests/it/typescript.rs:598`; removed-export import at `game/new-app.test.mjs:13`; transformation test at `host/web-js/build.test.mjs:352–385`. |
| M1 can become empty solely by implementing D1 | False | Unrelated versions at `Cargo.lock:721,2446`. |
| No lane step otherwise touches Snapback after moving `harmony` | False without further changes | The glue list includes `harmony-seed` (`scripts/async.mjs:118`); that test directly resolves and starts the server (`scripts/harmony-seed.test.mjs:20–26`). Delegate this through Harmony too. |
| Upstream crates are unpublished; seven releases followed the pin; Expo ships the described ABI/framework | Unverifiable here | Upstream repository/release evidence is absent. Local `publish = false` at `snapback4/Cargo.toml:27` describes this adapter. |
| Snapback can supply the wider ABI, private runtime/SQLite libraries and compatible retained partitions | Unverifiable here; central dependency | These are §4 asks, not existing repository capabilities. M2, M5 and M6 must establish them. |

## Delta assessment

Judging from the supplied diff alone, r2 resolves the main seam mistake: it replaces `Package` composition with the existing `NativeModule` slot, centralizes storage admission, makes the library declaration concrete, and requires fetching and hash verification. Shipping wasm glue and deferring Android are coherent improvements. Copying before deletion and waiting for all five Snapback asks repairs the cross-repository ordering. The deletion list is substantially better but remains incomplete; that is a material pre-existing concern. The measures improve through rebuilding and retained-partition reopening, but M1 introduces an impossible pass condition. The trait change also introduces an unacknowledged caller migration. The round therefore has not converged.

NOT READY — r2 fixes the native seam, but the transition omits existing consumers and M1 cannot pass as written.
## r3 (2026-10-31, delta round, the last)

**Provenance.** Family: OpenAI, codex `gpt-6.1-sol`, reasoning effort `ultra`, session `01a19183-2f8c-7d81-8e49-0c9663d5971a`, read-only in the round-3 capsule: a `git archive` of `9cc57a3f47` with `llp/reviews/1154-*` removed, capsule hash `e3b0c44298fc1c14…`. Target: r3, sha256 `fbc7c28d2ad9…`. Brief sha256 `4c3ab8c8eeaed117…` (the r2→r3 diff, IN-DELTA/PRE-EXISTING classification, and that this is the last round). Blind as before. Not redacted. The text below is the reviewer's final message, unedited.

---

NOT READY — D3 relies on exclusive SQLite file ownership that the specified storage guard does not establish.

## Direction

The ownership decision and native seam are right. `NativeModule` is the slot the driver already uses; generated wiring and host-owned admission let the package avoid `ibex2` and `exact-runner`. A package-owned C boundary and private dynamic library are feasible. The transition order is sound, and §4 now covers the wider client ABI, identity binding, entropy and callable preflight that step 3 needs. Status records approval for §3’s apparatus, consistent with `rules/RULES.md:63–66`. I verified the requested SHA-256 and reviewed only local sources; this capsule has no `.git`, so I could not execute M1 as written. Tags below follow the supplied r2→r3 diff.

## MATERIAL concerns

1. **[IN-DELTA] Directory authority does not establish exclusive database ownership.**

   **Where:** `llp/1154-exact2-and-snapback-move-independently.rfc.md:195–205`, `221–228`; §3 item 2.

   **What:** D3 defines `SqliteFile` as retaining directory authority and naming an admitted path, then says that guard ensures Exact’s SQLite never opens the partition. Those are different guarantees. The same `sqlite.open app:/data` grant can admit the partition through ordinary TypeScript storage while the package’s independent SQLite has it open. That violates the condition on which D3 justifies two SQLite libraries.

   **Evidence:** The existing `_directories` field retains directory capabilities (`snapback4/src/lib.rs:31–35`). `AppDirectories` pins directory handles (`vendor/ibex/crates/ibex2/src/stdlib/app_fs_unix.rs:17–23`); `resolve_sqlite` admits and resolves a filename (`:960–988`). Ordinary storage independently resolves that filename and calls its provider (`vendor/ibex/crates/ibex2/src/sqlite_abi.rs:172–189`), which opens it (`vendor/ibex/crates/ibex2-sqlite/src/lib.rs:69–75`). None of these operations establishes the exclusion D3 attributes to the guard.

   **Resolution:** Make exclusive file ownership an explicit `NativeStorage` contract shared with Exact’s ordinary SQLite admission, held until the library closes its connection. Specify refusal in either opening order and prove it in the fixture, including release after close. This is a defective safety invariant as written, not a request to specify its implementation algorithm.

## Minor concerns

1. **[IN-DELTA] Correct the implementor breakdown.** RFC `:203–205` and `:280–282` correctly say nine implementors but enumerate only eight. There are **five** test implementations: `js/tests/it/native_later.rs:18,79`, `storage.rs:106`, `turn_budget.rs:18`, and `interrupt.rs:76`. The same-commit migration decision is otherwise sufficient.

2. **[IN-DELTA] Finish the deletion inventory’s dependent imports and tests.** Besides the named preflight test, survivors include:
   - `game/new.mjs:7`, importing the deleted device helper;
   - `scripts/agent-launch.test.mjs:221`, importing that helper in the retained backend test;
   - `scripts/agent-launch.test.mjs:318–337`, testing Firefox/WebKit preflight through both the helper and `wireSnapback`.

   RFC `:307–321` should explicitly cover these. Rewrite the backend test around package dependency detection; remove the device-specific carrier test. These are routine deletion consequences, not a different design.

3. **[PRE-EXISTING] M1’s first grep also rejects permitted historical mentions.** RFC `:384` searches every occurrence of `expo/snapback`, while D1 `:111` prohibits dependencies on it. Closed issue explanations match at `issues/closed/20260924-root-lock-pins-private-snapback.md:11` and `20260928-apple-build-offline-metadata-on-fresh-clone.md:14`. Restrict that check to dependency inputs or distinguish historical matches. The supplied r2 side already has this mismatch.

4. **[IN-DELTA] M3 still needs an observation point.** RFC `:386` names an `engine_info` answer in drive state without saying how it gets there. The current driver’s call method is private (`snapback4/ts/snapback.ts:388`), and its engine probe supplies a request header (`:998–1000`). Name a Harmony-owned diagnostic source or a package-supported probe that Harmony exposes. Updating the lock before frozen installation fixes the earlier procedural problem.

5. **[IN-DELTA] Final linking needs more than a search path.** RFC `:229–234` correctly identifies the final Apple link, but implementation must carry the framework/library link arguments as well as search paths. The existing Swift target explicitly links the Rust archive (`host/apple/Package.swift:55–60`), and the product link has its own arguments and reuse fingerprint (`host/apple/build.mjs:1230–1247`). Include the package library in those inputs. Export isolation, deployment compatibility, install names, run paths and signing are implementation work that M5’s packaged launch should qualify.

6. **[PRE-EXISTING] Narrow the release claim.** RFC `:16–18` says every Snapback release needs an exact2 commit. Updating Exact’s compiled device needs that commit; merely publishing or consuming a compatible server release does not, as D4 itself recognizes. The current preflight compares a compatibility floor and capabilities rather than equality (`snapback4/web/device-build.mjs:63–66`).

7. **[IN-DELTA] Shorten Status’s review narrative.** RFC `:4` retains reviewer names and prior verdict history. Keep the current revision, approval and implementation assignment; the narrative adds little to the decision and conflicts with the sparse-prose guidance in `AGENTS.md:51–55` and the revision-ledger prohibition in `rules/DEFERRED.md:1704–1708`.

## Claims checked

| Claim | True / false / unverifiable | Evidence |
|---|---|---|
| Three Snapback crates are pinned to one immutable revision and exact version. | True | `snapback4/Cargo.toml:15–17`. |
| Root and Harmony install `snapback4@0.4.33`. | True | `package.json:13`; `apps/harmony/package.json:6`. |
| The receipt hashes nineteen files and records four README examples plus the auth example. | True | `scripts/check-snapback-readme.mjs:8–12,29–32`; `snapback4/readme-tested.json:7–53`. |
| Native Harmony uses `with_native` wrappers around the adapter. | True | `apps/harmony/app/apple/src/lib.rs:9–35`; `apps/harmony/app/linux/src/main.rs:10–36`. |
| The factory receives grants text; storage configuration occurs during activation. | True | `js/src/lib.rs:665–686`; `js/src/native.rs:44–56`. |
| The current adapter admits SQLite access and retains directory authority. | True | `snapback4/src/lib.rs:151–179,195–199`. |
| That retained authority prevents Exact’s SQLite opening the same file. | False | Directory authority and ordinary SQLite opening are independent; evidence in material concern 1. |
| There are nine trait implementors, including four test fakes. | False as stated | Nine total, five test implementations; minor concern 1. |
| `Package` is the wrong existing synchronous seam. | True | Its default synchronous call returns `NotSupportedError`: `host/linux/src/package.rs:29–43`. |
| Existing Rust package composition requires a generated Cargo manifest. | False if inferred | The built path uses `#[path]` modules and app-provided crates: `contract/cli/src/native/packages.rs:13–17,83–117`; LLP 1133.001 `:442–469`. RFC §3 can use that model for this shim. |
| `tally` proves source composition rather than prebuilt-library loading. | True | `packages/fixtures/tally/linux/exact.json:9–10`; `test/linux.mjs:21–35`. |
| Arbitrary package web assets need additional capture/serving work. | True | Package capture is import-driven: LLP 1027 `:1034–1040`; current asset copying is app-owned: `host/web-js/build.mjs:805`. |
| Harmony’s Apple crate is an rlib and final Apple linking needs host build integration. | True | `apps/harmony/app/apple/Cargo.toml:9–10`; `host/apple/Package.swift:55–60`. |
| Exact bundles SQLite. | True | `vendor/ibex/crates/ibex2-sqlite/Cargo.toml:12`. |
| Harmony’s manifest names iOS, macOS and web; its lane separately compiles its workspace. | True | `apps/harmony/app/app.json:12–29`; `scripts/async.mjs:163–166`. |
| Current Harmony test helpers build the device and install only when missing. | True | `scripts/memory-backend-test.mjs:106–110`; `packages/background/test/harmony.mjs:59–62`. D5 intentionally changes this. |
| Snapback’s upstream crates are unpublished; its existing xcframework, C ABI, compatibility floor, release count and publication qualification have the described properties. | Unverifiable | Their authoritative implementation and release records are absent. Local pins and consumers corroborate usage, not those upstream guarantees. |

The design most depends on Snapback delivering **§4 item 3’s expanded module-and-client ABI**, rather than only its existing four device functions, and preserving retained partition identities and queued writes. The local behavior to preserve is concrete: `snapback4/client/src/partition.rs:6–40`, `snapback4/src/lib.rs:180–200`, and web persistence in `snapback4/web/src/lib.rs:74–94,154–172`. M6 appropriately tests both persistence paths.

## Delta assessment

From the diff alone, r3 resolves the trait-migration omission in substance, with the minor counting error above. It identifies the final bundle/link/signing work and explicitly amends the shared-SQLite decision, but introduces the unsupported exclusivity claim that remains material. The Snapback asks now cover the shim, app identity, partition binding, entropy and callable preflight; I found no additional external deliverable step 3 clearly requires. The deletion list substantially improves but leaves dependent imports/tests. M1 still has historical false positives; M3 fixes lock-update ordering but needs a concrete state observation. M2, M5 and M6 otherwise measure the intended behavior. **No pre-existing MATERIAL concern remains in this review.** The SQLite ownership invariant requires a targeted design correction; the other findings are implementation or editorial details. As this is the last permitted round, that distinction should guide the author’s decision without another review loop.

NOT READY — D3 relies on exclusive SQLite file ownership that the specified storage guard does not establish.