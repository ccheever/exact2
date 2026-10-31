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