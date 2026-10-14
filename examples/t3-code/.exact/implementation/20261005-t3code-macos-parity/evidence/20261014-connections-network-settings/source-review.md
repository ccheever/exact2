# SERIAL3 final source review

**SOURCE APPROVED for the corrected 28-file candidate below. The visible grip ring, OS keyboard drop/cancel and Remove-dialog Escape focus repair are accepted on the reviewed evidence.** The authorized custom-button correction has no remaining source objection in this bounded review. The earlier missing-indicator finding is closed for this candidate and retained below as history. Physical OS pointer drag, normal process relaunch, route persistence across a normal relaunch and spoken VoiceOver output remain unverified. Native-agent pointer reorder passed separately. This is bounded repair acceptance, not complete task acceptance. Earlier source approval for the defective `833746...` candidate remains withdrawn.

The corrected candidate manifest is `target/connections-network-settings/source-candidate.json`, prepared against base `e9f702d851b76d61525938e2d0acecc141173a83`. All 28 file hashes and the aggregate were independently recomputed and matched. The approved source aggregate is:

```text
af34fbd9753f1d09c0c618ef90623dd64295ccbd8d5fd98b4f74872e06184ef8
```

Its exact recipe is SHA-256 of the UTF-8 concatenation of sorted entries, each `path + NUL + hexadecimal file SHA-256 + LF`. It is not the hash of JSON serialization. Approval applies only to the hashes in the table and the evidence scope below. Any subsequent source change needs a new frozen manifest and bounded review. The affected pre-grip candidate was `833746bc72383cebdb5ec50d320e5087630e797860dc70dcba319adc39c89945`, preserved in `source-candidate-before-focus.json`; its routes Contract hash was `accedfe5af80de750a3afe1c92d058dcd3d141f8c637e10a737b9dbb00ebd8d7`. Earlier certifications were `1fceafc91ba2fbc04fab47b9e52d2cdbfcefb07c71dbe0bf5e1014036a387719` (26 files) and `08a483637564225503db7d620326a792637614954fe3d4f5c5a501e169dea429` (28 files).

## Earlier finding: visible grip focus, now resolved on af34

On candidate `833746...`, the first route grip was the focused element but had no visible ring, outline or other focus treatment. `physical2/os-input/route-tab-grip-observation.json` identifies node 2605, stable ID `environment-route-grip-3d5442d9-8f96-421c-86ee-37dd9a34f877-http://127.0.0.1:16572`, labelled “Reorder This device, position 1”. The action snapshot independently names focused AX element 79 with that label. The reviewer viewed `route-tab-grip.png` and `routes-no-focus.png` at original resolution: the first grip has the same unmarked presentation. In `remove-escape.png`, a Remove control visibly has the blue focus ring. Thus keyboard focus identity and a visible focus indicator are distinct, and the latter was missing on the grip. The action envelope labels synthetic input verification as unverified; the focus conclusion comes from the accompanying focused-element observations, not an assumption that `ok:true` alone proves focus.

The root cause on the affected `833746...` candidate is grounded in the app and host source read at that snapshot:

- `connections-routes.contract:181` authors the grip as a `box` with `role="button"`, `tabindex=0`, key/blur handlers and `reorderFor`, but no press handler.
- `Accessibility.swift:12` defines `isButton` by actual button kind or native-button status; an ARIA role alone does not satisfy it.
- `Mac/NodeViewMac.swift:232` defines `pressable` through a press handler, link, or a real button's command/popover action. Key/blur/reorder handlers do not qualify.
- `Mac/FocusMac.swift:131` draws a custom ring only for `!isNativeButton && (isButton || pressable)`. This grip fails that test even when its keyboard focus is visible, so the mask bounds are empty. This matches the screenshots.

This was a demonstrated app defect against the prepared visible-focus requirement, not a focus-observation failure. The earlier source approval missed it. The authorized source correction and final evidence are reviewed below. The visible-indicator finding is now closed; physical OS pointer drag remains a separate open qualification. No app/framework source, test, Cargo/native process or OS input was changed or run by this reviewer during these certification refreshes.

The user explicitly authorized one additional grip repair round after the repository's three-round limit was reached. That bounded correction is accepted with the limits below. Separately, the coordinator reports that normal relaunch qualification stopped because Orca activation repeatedly opened an older registered bundle despite explicit current-path launches. That is a qualification setup blocker, not evidence of a T3 process-relaunch failure. The coordinator/implementer owns restoration of the original environment. This reviewer did not perform a fresh OS restoration comparison; the final independently read coordinator receipt records matching original typed preferences and root inventories, as distinguished below.

Evidence under `target/connections-network-settings/physical2/os-input/` was read without modification:

| Artifact | SHA-256 |
| --- | --- |
| `route-tab-grip.png` | `efdaf8c636e41725fec18b32f9be52daf794bac4c39163fb1402277f339596a3` |
| `routes-no-focus.png` | `d79b59469d4d5659965f6c07c145f20a3c380072444dc42e5746534f1f27685e` |
| `remove-escape.png` | `255b2f2ed4fd106bfa582d5612b1ba5704277503bc1d782b1438e28649b76640` |
| `route-tab-grip-observation.json` | `b87f7c33491ae3149d7ec55c5e5be502f36e3206ec3f990ec1c8580aa06b3880` |
| `route-tab-grip-action.json` | `1425df98c5740430ed1ddbebe2d6f9fdfe60f3429263b3dd8c1a0b0d56281c2f` |

## Authorized grip correction: bounded source review

Only `connections-routes.contract` differs from the affected `833746...` candidate; all other 27 file hashes match. The one declaration changes `box` to `button`, adds `appearance="none"`, `padding=0`, `border-width=0`, transparent background and `title=""`. The route identity, tabindex, role, label, key/blur handlers, `reorderFor`, disabled condition, dimensions, flex placement and glyph remain unchanged. Reversing exactly those declaration changes in memory reproduces the preceding Contract SHA-256 `accedfe5af80de750a3afe1c92d058dcd3d141f8c637e10a737b9dbb00ebd8d7`. Removing only `title=""` reproduces the interim Contract hash `8f3d880bbf8523b7e743b08aefdab0a2f35db2109c316e4767e8182c94312f3e` from candidate `2ed469...`.

The inspected source supports the correction without introducing another interaction path:

- `contract/lower/src/buttons.rs` explicitly returns `native:false` for `appearance="none"`; the existing native-button test expects a custom `Pressable` node for that declaration. This test was read, not rerun. The resulting custom button satisfies `isButton`, so `FocusMac.ringsFocus` permits the existing ring. Explicit zero padding/border and the transparent background retain the authored 1.5 rem grip box.
- The custom node retains the existing `gripKey` handler and its `preventDefault`/`stopPropagation`. It avoids `NativeButtonMac`'s Space activation branch. `ReorderGroup.reorderKeys` still refuses its automatic keyboard path because an authored key handler is present; there is still one app-owned keyboard transaction.
- `MouseReorderMac.down` chooses a nonempty `reorderFor` handle before its press/field exclusions, without a button-kind exclusion. `ReorderHold` uses that handle, its input eligibility and owning collection. The unchanged `reorderFor`/disabled/touch-action declarations therefore retain the existing pointer entry path at source level. This is not a native pointer-drop pass.
- `title=""` preserves the app's existing rule for labelled glyph-only buttons, suppressing an automatic help tag. The coordinator's whole-app check caught its absence in interim candidate `2ed469...`: 5,095 passed, 1 skipped and 1 failed, specifically the existing help-tags invariant. The test was left intact; the final property addition is the only delta from that interim source.

The reviewer read `round6-focus-bun.log` (46 passed, 0 failed, 158 assertions) for the interim conversion and the final `bun-app-final-focus-title.log` ending (5,096 passed, 1 skipped, 0 failed, 120,755 assertions across 335 files). The independently read `app-checks-final-focus-title.json` binds those passing whole-app checks and strict TypeScript exit 0 to candidate `af34fbd9753f1d09c0c618ef90623dd64295ccbd8d5fd98b4f74872e06184ef8`, with all 1,207 app source hashes unchanged before/after. The current compiled Rust, native build and coordinator results are recorded below. No new test or generic retest was requested or run by this reviewer.

## Final native evidence and bounded acceptance

The reviewer viewed all four regional screenshots below at original resolution and read the corresponding action receipts, observations and raw native tree/state files under `focus-repair/os-input/`. The native PID was 82736. For all 11 examined stages (`tab-grip`, `lift`, `move-down`, `drop`, `cancel-lift`, `cancel-move`, `cancel`, `remove-tab`, `remove-open`, `remove-escape`, `pointer-index-attempt`), the raw tree's focused nodes and dialog presence and the raw state's transaction, Settings visibility and saved route order matched the observations. Action envelopes alone were not treated as evidence of a successful interaction; the acceptance uses the resulting state, native focus, AX snapshots and screenshots together.

| Behavior | Observed result |
| --- | --- |
| Tab to the grip | Node 2420, the same stable route ID ending in `http://127.0.0.1:16572`, is focused at position 1. The screenshot shows its blue ring. |
| Space, Down, Space | Lift and movement retain focus on node 2420. Preview moves it to position 2 while saved order stays unchanged. Drop clears the transaction and changes saved order from `[16572, 16571, localhost:16571]` to `[16571, 16572, localhost:16571]`; the same grip remains focused and visibly ringed at position 2. Numeric entries abbreviate `http://127.0.0.1:<port>`. |
| Space, Up, Escape | The second lift previews the same route at position 1 without changing saved order. Escape ends the transaction, restores the saved rendered order and keeps node 2420 focused with its ring at position 2. Settings remains open. |
| Tab to Remove, open, Escape | Node 2491, the stable Remove button for the same route, is focused before opening. Cancel node 2616 is focused in the dialog. Escape removes the dialog and restores focus to the exact opener node 2491 with a visible blue ring; Settings stays open and all three saved routes retain their order. This supplies the actual-focus evidence that the earlier structural regression could not establish. |

These records establish the visible ring and the exercised OS keyboard behavior for the corrected candidate. Announcement strings are present in the transaction state, but this review does not infer that VoiceOver spoke them. The captures contain only the generic loopback fixture.

| Final screenshot under `focus-repair/os-input/` | SHA-256 |
| --- | --- |
| `tab-grip-region.png` | `cc1d544bd733058b48944e7b4093c91cf498863ca8413e517879e32300e09d75` |
| `drop-region.png` | `881d69d238f3f3ae5f0576616963f74da00b021abe01eb8b6f5a7e7f2757fb11` |
| `cancel-region.png` | `881d69d238f3f3ae5f0576616963f74da00b021abe01eb8b6f5a7e7f2757fb11` |
| `remove-escape-region.png` | `e5ee5744fc32444db6ec6435b67a1087d9c9796ec4227befdaa9e3473e297ebf` |

**Physical OS pointer drag remains UNVERIFIED.** Neither `pointer-drag` nor `pointer-index-attempt` changed the saved or rendered route order. The raw native results preserve that outcome; no root cause is established here. An `ok:true` input receipt does not turn either attempt into a pass.

Separately, the read `focus-pointer/pointer-summary.json` records a fresh current-candidate native-agent pointer pass on PID 92473: the existing phased platform drag changes `[16572, 16571, localhost:16571]` to the expected `[localhost:16571, 16572, 16571]`. Its reply explicitly identifies `delivery="platform"`, `carrier="macos"`, `mode="agent"`; the fixture ran with `EXACT_AGENT=1`. This supports preservation of the existing native pointer path but does not qualify either OS computer-use drag or persistence across normal process launch.

The read `focus-pointer/cleanup.json` records the driver closed. The read `focus-repair/final-cleanup.json` records PIDs 82736 and 92473 exited, identity-checked fixture backend/proxy PIDs 65781 and 65782 terminated, and fixture ports 16570–16572 free. These are implementer cleanup receipts, not a fresh OS inspection by this reviewer. The later coordinator-owned `cleanup-final-focus-independent.json` independently records both original preference domains equal in typed values with no extra keys (0 and 7 keys), all four original data/cache/temporary/KV inventories equal in contents, types, modes, owners and mtimes, the same four PIDs exited, fixture ports free, the three exact fixture Keychain accounts absent before and after, and the coordinator keep-awake process preserved. This reviewer read that receipt and verified its copied evidence file matches the coordinator copy; the coordinator performed those environment checks.

The final evidence bundle under `examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/20261014-connections-network-settings/` includes the read `physical-focus-title-independent.json` (SHA-256 `956ae969605beace7776cce36d83dcf561bd9a304de42432f682faeccd3b3b56`) and `cleanup-final-focus-independent.json` (`9a2d8655f2242cb94ebe9c20136c35636b460d701c969cf3935ee58046d06dc3`). Both copied files match their coordinator originals. The read `native-focus-final.json` hashes to `614b6e13561ea03581894d775b0a20f73383f1816bdd384baa806746c41e5b92`, binds candidate `af34...`, includes these independent references and preserves all remaining qualification limits. The coordinator reports matching its 140 private raw hashes and five published images; this reviewer separately verified the four focus screenshot hashes listed above. No OS-drag root cause is established by this review.

## Review and corrections

All 13 files in `kw10-early-review.md` match their independently reviewed/tested hashes. Of the 15 entries in `g2-early-review.md`, 11 candidate files match, the routes Contract and compiled-plan fixture changed, and two unchanged domain/helper-test files are outside the candidate. Those two also match their earlier hashes. The network-view comment correction and legacy-sidebar command-result fixture were read in full for the first certification. The Escape refresh added `app-settings.contract`, `composer-chip-popover.test.ts` and shortcut assertions. The subsequent Remove-route refresh changed only the modal property and added one regression. The latest grip-only delta is reviewed above; all prior corrections and failed attempts remain recorded here.

- **KW-10:** accepted settings writes request public Contract relaunch only after persistence. Rejected/no-op writes do not relaunch; the bounded accepted-write observer preserves relaunch across a superseded reply while retaining the stale handler's existing abort. The earlier reproduced lost-relaunch finding is resolved at the reviewed hashes. In-place settings restarts/rollback were removed; unrelated native lifecycle ownership is retained.
- **KW-3:** the native number field retains bounds, step, input, Enter and Shift-Tab behavior. The obsolete app arrow-step interception is removed. Digits-only validation still accepts `0443`. Tailscale confirmations now explicitly say that T3 Code restarts, and the network-view comment describes persistence followed by the public Contract relaunch.
- **G2:** the actual rendered route list follows the identity-based preview, with stable grip IDs/labels and a mounted polite live region. Baseline validation, one drop request, serial-correlated completion, owner loss, cancellation and pointer overlap guards remain as reviewed. Fresh saved-route validation precedes writes. Late/unrelated replies cannot announce success or restore focus. Remove-route retains the predecessor's dialog focus implementation.
- **Readiness correction:** the first source pass missed `settingsOpen && !modal` even though `modal` included `settingsOpen`. All seven new compiled tests exposed that contradiction at their shared ready precondition. The corrected explicit guard excludes every non-Settings term of the old modal expression, retains page focus/visibility and connection removal checks, and allows the Connections page itself. Open/close/mount/inert and owner transitions were traced; no second contradictory dependency was found. This review does not treat the earlier failed run as behavior evidence.
- **Fixture correction:** the later 35-pass/1-fail run failed the changed-route refresh case. The fixture had called `providerTick`, which only updates its clock for Providers or a ticking Network view. The final fixture advances time and calls the real `tick` action, which changes `elapsed` and therefore the `connectionsPage` resource input. It additionally asserts `routeKeyboardOwned == false` before waiting for cancellation. This repairs stimulus and proves the changed baseline was observed; it does not weaken the cancellation or zero-route-write assertions. The full final fixture, borrow fixes and formatting were read. The legacy fixture's four result fields match `Change` (`revision`, `message`, `relaunch`, `routeReorder`) and retain its assertions.
- **Escape correction:** the earlier certification and direct grip-handler tests missed Settings Back's registered Escape shortcut. The implementer's native probe found that it closed Settings before the grip could cancel. Independently reading `KeyEvents.swift` confirms that registered shortcuts run before the focused key handler. `SettingsWindow` now imports and injects the existing root-provided `RouteKeyboard`, and includes `routeKeyboard.phase != ""` in its existing `escapeOwned` expression. That value reaches `SettingsNav.menuOpen`, which removes Back's Escape shortcut throughout lift and saving. All previous Escape owners remain. No new state, shortcut handler or dismissal path was introduced. The actual-plan regression checks idle Escape availability, its absence while lifted and saving, and restored idle Escape with Settings still open after Escape/blur/unchanged-drop endings. Removing only these new assertions/comments in memory reproduces the exact prior fixture hash, proving that its other expectations were retained. The existing composer-chip source assertion now checks the five relevant owner terms independently of order/prefix, retaining the chip, palette, model and menu terms and adding the route transaction. That source/compiled-plan pass did not establish native Escape behavior; the final OS evidence above supplies the exercised cancel result.
- **Remove-route modal correction:** the next implementer probe passed route cancellation and the subsequent Settings Escape, then found that Escape inside Remove route still closed Settings. The earlier review missed that `role="alertdialog"` did not supply the modal property required by this host's shortcut filter. Independently reading `Shortcuts.swift` confirms that background shortcuts are excluded only behind a visible `accessibilityModal == true` view. The sole production delta adds `aria-modal=true` to `RouteRemoveDialog`; removing that property in memory reproduces the exact preceding Contract hash. The new actual-plan regression checks the modal property, Cancel's Escape shortcut inside the modal and Back outside it, then dispatches Cancel and checks Settings stays open, the dialog closes, the same opener node and saved route labels survive, and no command writes routes. Removing just this new test in memory reproduces the preceding fixture hash. Existing `DialogFinalFocus` and opener identity remain untouched. The regression does not execute native Escape routing or prove actual focus restoration; a surviving opener node is not a focus assertion. The required native evidence must establish actual opener focus before opening the dialog, then read it after Escape; the final records above now supply both. The earlier tap with empty opener focus cannot establish that behavior. This was the third bounded behavior repair; the user subsequently authorized the single additional grip repair described above.

## Verification boundary

The prior independent runs remain valid at their recorded snapshots: **86 Bun tests passed / 821 assertions** for KW-10 and **210 passed / 1,380 assertions** for the route/domain/dialog scope. Exact commands and results are in the two early reports. The 210-test run predates the guard/copy/fixture corrections and did not catch the readiness contradiction. No additional tests were executed by this reviewer for the certification refresh; the coordinator owns whole-app Bun/TypeScript and the implementer owns Cargo/native execution.

The reviewer read the actual `round3-rust-tests-overlay-guard.log`: **35 passed, 1 failed**, with the fixture refresh failure described above. The subsequently completed `round3-rust-tests-refreshed-fixture.log` reports **36 passed, 0 failed** in 76.90 s. That pass preceded the native-discovered Escape conflict and is not proof that it was absent. The reviewer then read `round4-rust-tests-escape-shortcut.log`, which reports **36 passed, 0 failed** in 76.40 s with the new shortcut assertions. Its source hashes matched the preceding `08a483...` candidate apart from the later composer-chip test addition. Both passes predate the modal property and new regression. At the superseding focus-indicator review, the reviewer read the completed `round5-rust-tests-remove-modal.log`: **37 passed, 0 failed** in 76.73 s. This result does not cover the missing visible grip indicator and does not resolve the finding.

The coordinator's earlier `target/connections-network/gates/app-checks-final-escape-fixture.json` and `source-provenance-escape.json` were independently read for candidate `08a483...`, with its Bun/TypeScript success and no framework changes. The preceding `app-checks-final-remove-modal.json` was then independently read with its Bun log ending: **5,096 passed, 1 skipped, 0 failed; 120,755 assertions across 335 files**, and strict TypeScript exited 0. That receipt binds candidate `833746bc72383cebdb5ec50d320e5087630e797860dc70dcba319adc39c89945` and records unchanged hashes across all 1,207 app files before/after. The reviewer separately recomputed all 28 current file hashes and their aggregate. Git-based provenance checks remain coordinator-owned.

For the final `af34...` candidate, the reviewer read `round7-focus-title-rust.log`: **37 passed, 0 failed, 0 ignored**, in 77.98 s. Its independently recomputed SHA-256 `2f629e368148f7ec7e04361dd350dd2a2a411af4d68b88d90a3a1d033facda07` matches the coordinator's `compiled-app-focus-title-independent.json`. The read `native-build-focus-title-independent.json` binds the same source aggregate, records all 28 source and nine build-artifact hashes matched, and identifies the 09:47:26.764Z development build. This reviewer read that coordinator verification receipt; the coordinator performed the nine-artifact verification.

The read `target/connections-network/gates/coordinator-checks.json` records unchanged framework inputs and successful earlier default-member build, test, clippy, format and boot checks. Its final-candidate caps, workspace check, format and all-app Contract corpus sweep all exited 0. It also binds the final whole-app Bun/strict TypeScript results and unchanged source hashes across the final integration checks. These are coordinator-run gates; this reviewer did not rerun them.

A compiled Contract relaunch command is not evidence of actual old-process exit and replacement. **Normal changed-PID relaunch, route persistence across a normal relaunch, physical OS pointer drag and spoken VoiceOver announcements remain unverified.** The existing activation setup blocker is preserved above. This bounded refresh does not independently certify native number stepping/AX behavior or offscreen grip scrolling. It does close the demonstrated visible grip indicator and exercised keyboard/Remove-focus findings using the final evidence, without claiming complete task acceptance.

This pass performed no Git command, Cargo invocation, native launch, OS input, source/task edit or external write. This ignored report is its only written artifact.

## Exact candidate files

| File | SHA-256 |
| --- | --- |
| `examples/t3-code/adopt-main-fixes-r10.test.ts` | `4d7b4bb80cde75a3608169f692fdd4d4ef87d7fb5599075d8c2fb7ff1ea22fef` |
| `examples/t3-code/app-settings.contract` | `e6f25ab03524228ff4db35d07b5a36cf431b606da8b468f12b768173185ea321` |
| `examples/t3-code/app.contract` | `1b764e97009bd8d168e5b9e19eebc7522b4bed1aaa704a305ed0b8ee64c26ae4` |
| `examples/t3-code/app.ts` | `5d4f3ae7af37d6a4011bc88e6bb33f33425fcedd4cdc8a0ec9693508dab21e6e` |
| `examples/t3-code/client-ops-connection.ts` | `f30619bc233e0d8957d89cc9cb94d68276617d99222c0475d61bfaa85bd0016f` |
| `examples/t3-code/client-ops.ts` | `0548fafefbbe8cb42c3f64ea6094fb226483d7ae6dfcbe87e50a12f3bdb4f7d4` |
| `examples/t3-code/client.ts` | `3c06509ef5a595a14344cc64433b48ff5138bb31d71801fc020ff258249dd0cd` |
| `examples/t3-code/composer-chip-popover.test.ts` | `21e16955ec867f0d5c4cab5ed74c21f9498bc3e60f4ec5b243782f56fd7b6207` |
| `examples/t3-code/connection-routes-ops.ts` | `435fe4393bf617a874f90f6ddc061809a5e13a7245b535651de96c5804e6e204` |
| `examples/t3-code/connections-network-dialogs.contract` | `42e0764103703486292d80ccfc183a320db10f81285193934457403a4c81d987` |
| `examples/t3-code/connections-network.contract` | `349e398a1fcaa10c497a7e3ed43aadc476fbd2220e7ff26297817016de71d546` |
| `examples/t3-code/connections-network.test.ts` | `a4627e45dd22add20452e9eeecb5054e336bc66a50b01c197cbc4417ab575070` |
| `examples/t3-code/connections-network.ts` | `9429002354232b6ca4805422eb495b297a79e014dbb174ce4d0ef41575ac8193` |
| `examples/t3-code/connections-routes.contract` | `b7422841fa060c81f36b493f6b568f0fd43280c0b10b057e6ad728e04cf78cfa` |
| `examples/t3-code/connections-routes.test.ts` | `bae8f1de88dfad98d6c1f844b1a538be4fada97a30b2be59bf90f5ac4e1db28f` |
| `examples/t3-code/connections.contract` | `e2fb8d04094050a72fca899e29d5ef38bff3d46065c33ae4122c0309ff3a09c2` |
| `examples/t3-code/connections.ts` | `54623fe517c9dd4c5f7b42b883e9c1fa8433b9175227753d34fccb6224721a6e` |
| `examples/t3-code/local-primary.test.ts` | `65e1fb5f41bc0e0377d40c67fd000e593dd4ac4a219c6bf0147fd8036341d742` |
| `examples/t3-code/macos/src/composer_accessibility_tests.rs` | `4f4a91022b4ad9f70ab3352d64afc3fe08360f17a88c8dba09e6432fd4f0dd7c` |
| `examples/t3-code/macos/src/connections_settings_tests.rs` | `c8044a87338a71c062602fce7b16fb0efcc9afc6b7176f599782420e9ed89f6a` |
| `examples/t3-code/macos/src/legacy_sidebar_tests.rs` | `f4d1a8f826655e57d4eab89489f76abf971a1f9e8f1df194b8af868288a47246` |
| `examples/t3-code/macos/src/lib.rs` | `d1e0c8e958b8035551dca4483ec026081af771dff8817ed1d32c20b785f39e23` |
| `examples/t3-code/network-routes-followups.test.ts` | `6bc885e6752be09b39c0c7c8a66d1f60709482765381caa0bac4efe69df25d83` |
| `examples/t3-code/providers-scope.test.ts` | `5dbcd775545505e15a0a7726b04ca9eacd46cfc255fa05f8365887dfe169b6c5` |
| `examples/t3-code/server-exposure.test.ts` | `eb1e509d19fe71d85f211a8fc228cb8550d58cc73651937b51e8a328e40e4720` |
| `examples/t3-code/server-exposure.ts` | `8f7b0816b935082a05ee44f8c806e4c17cc3e6555aa8feeb1c48b614067dcabf` |
| `examples/t3-code/shapes.contract` | `3f76031ec4780d8f782a633c7bdf79de15cb116f255fe7a23df4b37562a1b325` |
| `examples/t3-code/this-machine.ts` | `8c088f2412b75b6f0023f1d9853eef73fe2694222dd44901a07738f5d7fc7491` |

Additional unchanged reviewed dependencies: `connection-routes.ts` = `bfde4cea91b75b9a999c315d941da1c2bbdaf5e8d0640dbfdfb129961c01cbb2`; `connection-routes.test.ts` = `ddc7628fad286ad6635d105bd45dd079d13f17193ffcf4733c47653f3adc0105`.
