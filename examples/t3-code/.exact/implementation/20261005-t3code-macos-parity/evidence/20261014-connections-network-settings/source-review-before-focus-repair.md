# SERIAL3 final source review

**SOURCE APPROVED at the 28-file candidate below. No unresolved source finding.** This supersedes the earlier certifications after native probes exposed both the active-route Escape conflict and the Remove-route dialog's missing modal declaration. The latest compiled-plan run and final native build/acceptance remain pending and coordinator/implementer-owned. This certifies the reviewed source, not full task acceptance. Normal-process replacement, physical focus and spoken announcements remain unverified under the user's no-screen direction.

The candidate manifest is `target/connections-network-settings/source-candidate.json`, prepared against base `e9f702d851b76d61525938e2d0acecc141173a83`. Every file's SHA-256 was independently recomputed and matched. The independently verified aggregate is:

```text
833746bc72383cebdb5ec50d320e5087630e797860dc70dcba319adc39c89945
```

Its exact recipe is SHA-256 of the UTF-8 concatenation of sorted entries, each `path + NUL + hexadecimal file SHA-256 + LF`. It is not the hash of JSON serialization. Approval applies only while these 28 hashes match; changed source requires a bounded refresh. The superseded 26-file manifest remains in `source-candidate-before-escape.json` at aggregate `1fceafc91ba2fbc04fab47b9e52d2cdbfcefb07c71dbe0bf5e1014036a387719`. The immediately preceding 28-file certification was `08a483637564225503db7d620326a792637614954fe3d4f5c5a501e169dea429`.

## Review and corrections

All 13 files in `kw10-early-review.md` match their independently reviewed/tested hashes. Of the 15 entries in `g2-early-review.md`, 11 candidate files match, the routes Contract and compiled-plan fixture changed, and two unchanged domain/helper-test files are outside the candidate. Those two also match their earlier hashes. The network-view comment correction and legacy-sidebar command-result fixture were read in full for the first certification. The previous refresh added `app-settings.contract`, `composer-chip-popover.test.ts` and Escape assertions. Relative to that immediately preceding 28-file candidate, this refresh changes only the routes Contract's modal property and adds one regression; the other 26 files match exactly.

- **KW-10:** accepted settings writes request public Contract relaunch only after persistence. Rejected/no-op writes do not relaunch; the bounded accepted-write observer preserves relaunch across a superseded reply while retaining the stale handler's existing abort. The earlier reproduced lost-relaunch finding is resolved at the reviewed hashes. In-place settings restarts/rollback were removed; unrelated native lifecycle ownership is retained.
- **KW-3:** the native number field retains bounds, step, input, Enter and Shift-Tab behavior. The obsolete app arrow-step interception is removed. Digits-only validation still accepts `0443`. Tailscale confirmations now explicitly say that T3 Code restarts, and the network-view comment describes persistence followed by the public Contract relaunch.
- **G2:** the actual rendered route list follows the identity-based preview, with stable grip IDs/labels and a mounted polite live region. Baseline validation, one drop request, serial-correlated completion, owner loss, cancellation and pointer overlap guards remain as reviewed. Fresh saved-route validation precedes writes. Late/unrelated replies cannot announce success or restore focus. Remove-route retains the predecessor's dialog focus implementation.
- **Readiness correction:** the first source pass missed `settingsOpen && !modal` even though `modal` included `settingsOpen`. All seven new compiled tests exposed that contradiction at their shared ready precondition. The corrected explicit guard excludes every non-Settings term of the old modal expression, retains page focus/visibility and connection removal checks, and allows the Connections page itself. Open/close/mount/inert and owner transitions were traced; no second contradictory dependency was found. This review does not treat the earlier failed run as behavior evidence.
- **Fixture correction:** the later 35-pass/1-fail run failed the changed-route refresh case. The fixture had called `providerTick`, which only updates its clock for Providers or a ticking Network view. The final fixture advances time and calls the real `tick` action, which changes `elapsed` and therefore the `connectionsPage` resource input. It additionally asserts `routeKeyboardOwned == false` before waiting for cancellation. This repairs stimulus and proves the changed baseline was observed; it does not weaken the cancellation or zero-route-write assertions. The full final fixture, borrow fixes and formatting were read. The legacy fixture's four result fields match `Change` (`revision`, `message`, `relaunch`, `routeReorder`) and retain its assertions.
- **Escape correction:** the earlier certification and direct grip-handler tests missed Settings Back's registered Escape shortcut. The implementer's native probe found that it closed Settings before the grip could cancel. Independently reading `KeyEvents.swift` confirms that registered shortcuts run before the focused key handler. `SettingsWindow` now imports and injects the existing root-provided `RouteKeyboard`, and includes `routeKeyboard.phase != ""` in its existing `escapeOwned` expression. That value reaches `SettingsNav.menuOpen`, which removes Back's Escape shortcut throughout lift and saving. All previous Escape owners remain. No new state, shortcut handler or dismissal path was introduced. The actual-plan regression checks idle Escape availability, its absence while lifted and saving, and restored idle Escape with Settings still open after Escape/blur/unchanged-drop endings. Removing only these new assertions/comments in memory reproduces the exact prior fixture hash, proving that its other expectations were retained. The existing composer-chip source assertion now checks the five relevant owner terms independently of order/prefix, retaining the chip, palette, model and menu terms and adding the route transaction. This source/compiled-plan review does not claim the final native retest passed.
- **Remove-route modal correction:** the next implementer probe passed route cancellation and the subsequent Settings Escape, then found that Escape inside Remove route still closed Settings. The earlier review missed that `role="alertdialog"` did not supply the modal property required by this host's shortcut filter. Independently reading `Shortcuts.swift` confirms that background shortcuts are excluded only behind a visible `accessibilityModal == true` view. The sole production delta adds `aria-modal=true` to `RouteRemoveDialog`; removing that property in memory reproduces the exact preceding Contract hash. The new actual-plan regression checks the modal property, Cancel's Escape shortcut inside the modal and Back outside it, then dispatches Cancel and checks Settings stays open, the dialog closes, the same opener node and saved route labels survive, and no command writes routes. Removing just this new test in memory reproduces the preceding fixture hash. Existing `DialogFinalFocus` and opener identity remain untouched. The regression does not execute native Escape routing or prove actual focus restoration; a surviving opener node is not a focus assertion. The coordinator's final native probe must establish actual opener focus before opening the dialog, then read it after Escape. The earlier tap with empty opener focus cannot establish that behavior. This is the third and final bounded behavior repair; further native production failures must be reported without another implementation loop.

## Verification boundary

The prior independent runs remain valid at their recorded snapshots: **86 Bun tests passed / 821 assertions** for KW-10 and **210 passed / 1,380 assertions** for the route/domain/dialog scope. Exact commands and results are in the two early reports. The 210-test run predates the guard/copy/fixture corrections and did not catch the readiness contradiction. No additional tests were executed by this reviewer for the certification refresh; the coordinator owns whole-app Bun/TypeScript and the implementer owns Cargo/native execution.

The reviewer read the actual `round3-rust-tests-overlay-guard.log`: **35 passed, 1 failed**, with the fixture refresh failure described above. The subsequently completed `round3-rust-tests-refreshed-fixture.log` reports **36 passed, 0 failed** in 76.90 s. That pass preceded the native-discovered Escape conflict and is not proof that it was absent. The reviewer then read `round4-rust-tests-escape-shortcut.log`, which reports **36 passed, 0 failed** in 76.40 s with the new shortcut assertions. Its source hashes matched the preceding `08a483...` candidate apart from the later composer-chip test addition. Both passes predate the latest modal property and new regression; neither is a pass for the current candidate. At this certification refresh, `round5-rust-tests-remove-modal.log` was still compiling.

The coordinator's earlier `target/connections-network/gates/app-checks-final-escape-fixture.json` and `source-provenance-escape.json` were independently read for candidate `08a483...`, with its Bun/TypeScript success and no framework changes. The current `app-checks-final-remove-modal.json` was then independently read with its Bun log ending: **5,096 passed, 1 skipped, 0 failed; 120,755 assertions across 335 files**, and strict TypeScript exited 0. This latest receipt binds candidate `833746bc72383cebdb5ec50d320e5087630e797860dc70dcba319adc39c89945` and records unchanged hashes across all 1,207 app files before/after. The reviewer separately recomputed all 28 current file hashes and their aggregate. Git-based provenance checks remain coordinator-owned.

Native build success and coordinator repository gates must be recorded separately before landing. A compiled Contract relaunch command is not evidence of actual old-process exit and replacement. Native number stepping/AX behavior, physical grip focus/scrolling, spoken announcements, Remove-route Escape focus and persisted order across normal launch also remain separate qualifications.

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
| `examples/t3-code/connections-routes.contract` | `accedfe5af80de750a3afe1c92d058dcd3d141f8c637e10a737b9dbb00ebd8d7` |
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
