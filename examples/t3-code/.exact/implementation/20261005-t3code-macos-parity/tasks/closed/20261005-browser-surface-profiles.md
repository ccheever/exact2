---
name: 20261005-browser-surface-profiles
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-profiles
pr_url: https://github.com/ccheever/exact2/pull/354
verified_commit: a6e31eed74678d6cbdfef59892f369f034cf6b76
---

# Browser surface part 4: profiles, cookie import and clearing

## Outcome

Browser tabs open under the reference's profiles: the built-in Default and Incognito and up to 24 named ones, each with
its own storage; the "+" menu's profile submenu (part 1 lists Default only) and the launcher's chevron list them; a
tab in another profile than the default shows its badge in the chrome row; the More menu's "Profile: <name>" group
clears cookies and cache for that profile only (part 1 draws Clear cookies and Clear cache disabled, marked "Part 4");
Settings › Integrations › Browser manages the profiles and the default one; the cookie import wizard brings cookies
from Chrome, Edge, Brave, Vivaldi, Opera, Arc, Helium, Firefox and Safari; and the rest of Settings › Integrations ›
Browser's defaults group (coordinator scope addition, 2026-10-09, confirmed by the repo owner: the settings defaults belong
to part 4): the default viewport, zoom and appearance a new tab opens with, the recording frame rate and key/mouse
presses (part 3's client settings), Auto-show floating preview, beside part 5's "Open links in".

Split from [20261005-browser-surface](closed/20261005-browser-surface.md) at its `prepare` (planned split, part 4). It starts
after part 1 merges into `feat(example)/t3-code`.

## Scope and exclusions

From the parent's scope (its numbering):

1. **The profile submenu.** "+" menu › Browser › the profile list and the launcher's "Open browser in a profile"
   chevron (shown only with more than one profile, `RightPanelTabs.tsx:495-560, 1246-1285`); the tab's profile badge
   (`PreviewView.tsx` leadingActions, shown only when it differs from the default).
3. **More menu, part 4's group.** "Profile: <name>" with Clear cookies and Clear cache (`PreviewMoreMenu.tsx:196-228`,
   `BrowserSession.ts` `clearCookies`/`clearCache`: cookies, local storage, IndexedDB, service workers; the cache),
   scoped to the tab's environment and profile.
10. **Profiles and cookie import.** Built-in Default (persistent) and Incognito (in memory, cleared when the app quits),
    up to 24 user profiles (`BROWSER_PROFILE_MAX_COUNT`, names up to 48 characters; `browserProfile.ts:19-60`), the
    default profile for new tabs; Settings › Integrations › Browser's profiles row ("Browser profiles", "Profiles
    separate cookies and logins. Incognito data is cleared when the app closes.", Add profile; clone:
    `settings-source-control.contract` BrowserDefaults); the cookie import wizard with its six unavailable reasons and
    copy (`browserImport.ts:31-60,142-154`, `BrowserImportWizard.tsx`): "Quit <browser> to import", "Let T3 Code read
    <browser>'s cookies" (Full Disk Access), "Import from <browser>", "Importing cookies", "Couldn't import from <browser>".
    WebKit: part 1 already gives each environment's profile its own persistent `WKWebsiteDataStore` (identifier derived
    from the environment and the profile, `T3BrowserSessions.storeIdentifier`); Incognito is a non-persistent store; an
    import writes through the store's `httpCookieStore` (confirmed at `prepare`).

Added by the coordinator (2026-10-09): **the Browser defaults group** in Settings › Integrations › Browser
(`IntegrationsSettings.tsx` BrowserViewportSetting, the zoom, appearance, recording and Auto-show rows;
`browserDefaults.ts`): the default viewport (Fill, Responsive with typed width/height and rotate, the presets), zoom and
appearance, applied to every new page through part 2's `browserSet` and to Show device toolbar; the recording frame
rate, key presses and mouse presses (part 3's `CLIENT_DEFAULTS` keys, byte-identical); Auto-show floating preview, read
by part 5's automation host; part 5's "Open links in" row reused. Each row has the reference's reset.

Excluded: parts 2, 3 and 5 (part 4 wires its defaults into their open paths: part 5's link opens and agent tabs, part
2's device toolbar).

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`packages/contracts/src/{browserProfile,browserImport}.ts`, `apps/desktop/src/preview/BrowserSession.ts`,
`apps/desktop/src/preview/BrowserImport/*`, `apps/web/src/browser/browserDefaults.ts`). Part 1's seams:
`browser-surface.ts` `BROWSER_PROFILES` (the list the menu and the badge read), `T3BrowserSessions.swift` `store(…)`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](closed/20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| framework issue | [X66](../../issues/closed/20261008-x66-popover-from-action-and-toggle.md) | #319 | nonblocking: the profile submenu opens from its chevron, not on hover, until a popover can open from an action | — |
| task PR, reverted and re-landed | `20261005-browser-surface-navigation` (part 2) | [#348](https://github.com/ccheever/exact2/pull/348), reverted by #351, re-landed as [#352](https://github.com/ccheever/exact2/pull/352) | Part 2 re-lands on `feat(example)/t3-code`; this branch then merges the base again | re-landed as `ab220bfdb` (2026-10-10); merged here at `e075de6e6` |
| task PR | `20261009-fix-settings-integrations-loop` | [#353](https://github.com/ccheever/exact2/pull/353) | Settings › Integrations reads the subscribed config; part 4's Browser rows keep that | merged as `950e8e2e5`; merged here at `e075de6e6`, both sides kept |

## Acceptance and reproduction

Rows from the parent's table: "Open and tabs" (each profile), "More menu" (the Profile group), "Profiles", "Cookie
import" (fixture browser stores; no real browser data is read in the lane run). Tests to port (`bun:test`, original
names; counts from the parent): `packages/contracts` `browserProfile.test.ts` (9), `BrowserImport/*` (108; the reader
tests need the engine and fixtures: port the pure parts, run the rest as lane rows), `addBrowserSurface`'s profile cases
and `openPreviewSession`'s "does not open … with unread settings and uses the saved profile on retry". Standard gates
as the parent's.

### Results

Live rows ran in agent mode on fixture browser stores only (a fixture home under the worktree's `target/`, named by
`T3_BROWSER_IMPORT_HOME`; no real browser profile, Keychain item or Full Disk Access was read, asked for or granted).
After the 2026-10-10 review, every row ran again in one session at `eb8daccda` (drive 9, all 195 ops; the code head
`657ca0794` adds only the base's #371), with the same steps on the evidence base `950e8e2e5` (the feature-branch tip,
which has no part 4: the before column) and on the reference (T3 Code `1e2ecbd975`, Electron, `A/ref-app.sh` on 16750,
the same fixture stores in its lane home). Each image is before | after | reference, one row per step. The reference
cannot show three steps, said in its cell: Chrome's and Arc's imports read the login Keychain in-process, and Safari's
Full Disk Access step needs TCC's EPERM on the real Safari container; there the reference column is its configure step
or its source.

| Row | Result | Evidence |
| --- | --- | --- |
| Settings › Integrations › Browser: Browser profiles and the defaults group, beside part 5's "Open links in" | passed (agent mode, `eb8daccda`) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/e0cd8a20c6957bab922c56ca4daf7552be42cea1/browser-surface-profiles/r-integrations.png) |
| Add profile: its menu (Blank profile, Import from the installed browsers), a blank profile, rename | passed | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/a12ecb2f8f1fc8e3ca0273072f1b465ca338f23f/browser-surface-profiles/r-add-profile.png) |
| A profile's menu (Set as default, Clear cookies and cache, Remove profile and data), the remove dialog, removed (its stores cleared in the environment) | passed | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/df1b9cd5cdfef86aaa4a3dcd2b53b4ef3c2e8507/browser-surface-profiles/r-row-menu-remove.png) |
| Import from Chrome into a new profile: 3 imported, 2 skipped (a CHIPS cookie and a schema-24 cookie bound to another host) | passed; the reference's import was not run (Keychain), its configure step matches | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/1bc9bf99e1849a33ff1955c8435b939c153f56ee/browser-surface-profiles/r-import-chrome.png) |
| Brave running: "Quit Brave to import", then "I've quit it" with the browser gone opens the configure step | passed | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/82e12a887623215d6c19834fcbd7b59052e8bb36/browser-surface-profiles/r-import-brave.png) |
| Safari: "Let T3 Code read Safari's cookies" (the fixture's EPERM); Allow is only recorded outside the packaged build; the grant is read again every 1.5 s and on focus (review fix) | passed: agent state on the step read `fdaPoll` 2, then 4 after 3.2 s. Before (`24d5ad2e7`): read only when the page was drawn | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/f4378a7bef3732f5b0ccca322cea42e3990f5768/browser-surface-profiles/r-import-safari-fda.png), [record](https://raw.githubusercontent.com/ccheever/exact2/78b9d29a596ae182e056f5996b343ddf670cc4b7/browser-surface-profiles/drive-after-eb8daccda.txt) |
| Arc: the Keychain refused, "Couldn't import from Arc" with Try again | passed; the reference was not run past configure (Keychain) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/581b2ad022ae3b466f496d563917aeeb06f614df/browser-surface-profiles/r-import-arc.png) |
| Import from Firefox into the existing Default profile (the container cookie is not imported) | passed (the reference: "Imported 1 cookie", "Added to Default") | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/25a16864271c6460eaa9f030d084842148625cf7/browser-surface-profiles/r-import-firefox.png) |
| Default browser viewport's menu: Fill panel, Responsive, then "Standard" over the presets, each size right-aligned (review fix) | passed: the reference's rows and order; the popup 278 wide at its trigger's right edge, as measured in the reference. Before (`289861020`): the size inside each label, no heading | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/8d76a708ad8ff7db2d36cef3c9b4f5e220b45170/browser-surface-profiles/r-viewport-menu.png) |
| Default viewport, zoom, appearance, frame rate, key presses, Auto-show set and two reset | passed (the viewport and zoom took effect on the tabs below) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/8785893e46904a273ea0139c5c27f0bb8b122f39/browser-surface-profiles/r-defaults.png) |
| Settings › General's Restore defaults with only frame rate 60, zoom 125% and iPhone 12 Pro changed (review fix) | passed: the confirmation reads "This will reset: Browser viewport, Browser zoom, Recording frame rate.", as the reference's does; Confirm leaves Fill panel, 100%, System, 30 fps. Before (`24d5ad2e7`): no labels, Restore disabled | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/ec1b55d22405e8b564e65e7f54b7e21de263b53e/browser-surface-profiles/r-restore-defaults.png), [Bun before/after](https://raw.githubusercontent.com/ccheever/exact2/c913f31a92564eb95fc3876b3145a14d517e507c/browser-surface-profiles/restore-defaults.txt) |
| The launcher's chevron with more than one profile | passed | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/d2d28f674c6da10e0c5b731dfef5541a3d5deacc/browser-surface-profiles/r-launcher.png) |
| A tab in the imported profile: its badge, its cookies, the configured viewport, zoom and appearance | passed: the probe received `lane_chrome, lane_session` (the reference's Firefox profile: `lane_firefox`) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/80b2e5adb80300a2c97ec34b03667f4bb4725d3a/browser-surface-profiles/r-profile-tab.png) |
| More menu: "Profile: Chrome" with Clear cookies and Clear cache; Clear cookies empties that profile only | passed: after Clear cookies and a reload the probe received none (the reference too) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/9446243cf0864d0c4d8c007b351731884415a8c1/browser-surface-profiles/r-more-menu.png) |
| "+" › Browser › the profile list; Incognito's cookie stays in Incognito; a Default tab has Firefox's cookie only | passed: Incognito received `incognito_probe` after `/set`; the Default tab received `lane_firefox` only (the reference: the same) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/877bf0cce6d1d1c598d7d28c6ef5c172e54d3a96/browser-surface-profiles/r-plus-browser.png) |
| Settings › Integrations with no connected environment shows the Browser section (review fix) | passed (agent mode, the embedded server unable to start): the notice, then Browser profiles and the defaults rows; the reference draws the section unconditionally (source; its shell always runs its backend) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/d6a916c7b7eb325d6af8a5c38abe135ea05a1a15/browser-surface-profiles/r-integrations-offline.png), [after](https://raw.githubusercontent.com/ccheever/exact2/ef8f66027516918f551ebc1ed83000d062ddd314/browser-surface-profiles/drive-after-offline.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/c87a96b71d7b65f40dfd852e18860c2ed5f0e0ca/browser-surface-profiles/drive-before-offline.txt) |
| A chat or terminal link clicked while the settings are unread opens neither browser; a terminal link opens under the configured viewport and profile (review fix) | passed (Bun; `browser-links.test.ts`): `BrowserSettingsReadError`, no `preview.open`, no system browser. Before (`24d5ad2e7`): both went to the system browser | [Bun before/after](https://raw.githubusercontent.com/ccheever/exact2/724cf2f7befc8d43a41fd4bc2f3f16495a8b9e47/browser-surface-profiles/links-unread-settings.txt) |
| After part 2's re-land: a tab at the default viewport (iPhone 12 Pro) and zoom (125%) lays out its first page at the zoom (the default zoom in `liveSessions`' zoom field) | passed (agent mode, `289861020`, and `01da6c0d8`): part 2's first-layout fixture got 389 × 844 at devicePixelRatio 2.5 and no resize. Before (`e075de6e6`, the merge alone): 312 × 675 at 2.5, then a resize to 389 × 844. Reference: 1280 × 840 and 488 × 1055 at 2 (two loads), then 390 × 844 at 2.5 (its zoom lands after the load) | [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/b6aa6ac94afaf8c47df12eab518d4adcd6efc504/browser-surface-profiles/p4-first-layout-125.png), [before](https://raw.githubusercontent.com/ccheever/exact2/ef2b2b5afbc7482c94f7d0cc04913e46d6251cf2/browser-surface-profiles/drive-before-e075de6e6.txt), [after](https://raw.githubusercontent.com/ccheever/exact2/b640b82a1281dcd936fa76e0271899de0d133259/browser-surface-profiles/drive-after-289861020.txt), [reference](https://raw.githubusercontent.com/ccheever/exact2/b2cf62e1e5ddd8abd2583c8c7e5b2ce7e34e0ad2/browser-surface-profiles/reference-first-layout.txt), [Bun before/after](https://raw.githubusercontent.com/ccheever/exact2/c5173ebe043f994a72a78f27252536ea4f4da2df/browser-surface-profiles/test-default-zoom.txt) |
| The launcher's profile chevron by keys: Enter opens its list and not the launcher's lit row (Files); the list's Enter opens that profile's tab only; ↑ opens it and leaves the lit row; Incognito by Enter (review fix: a Contract test against the app) | passed: `browser-launcher-chevron.test.contract` 3 of 3 at `eb8daccda` (`agent.mjs macos --test`). With the chevron's key guards taken out, test 1 fails (Files opens on the chevron's Enter); tests 2 and 3 pass either way | [test runs](https://raw.githubusercontent.com/ccheever/exact2/38dcd0b86ccc10246b028c0fe92742f514fa33d1/browser-surface-profiles/test-chevron-contract.txt), [before/after/reference](https://raw.githubusercontent.com/ccheever/exact2/e8b5a417829b9d52f23004c80688b93b518ef459/browser-surface-profiles/p4-chevron-enter.png) |
| Settings › Integrations' Browser rows keep #353's reads: the page reads the subscribed config, and the rows add no server read | passed (Bun: `settings-integrations-reads.test.ts` now also checks the answer's Browser rows, with no `server.getConfig`, `server.getSettings` or `device.list`) | [checks](https://raw.githubusercontent.com/ccheever/exact2/19bb5e5f52b437c3b03e6d882977f8dde84fc2d3/browser-surface-profiles/checks-289861020.txt) |
| A tab made with its URL (a link, a script's preview, an agent's tab) at 125%: its first layout | not run live: no agent-mode path in this lane opens one (Files › Open file in preview browser on a draft thread fails at the server, `AssetWorkspaceContextResolutionError`; a chat link or an agent's tab needs a provider turn). `browser-surface.test.ts` ("part 4: a page the module has not reported yet …") checks the `browserSync` it sends, #352's AppKit `testAPageSyncedAtAFixedViewportLaysOutAtItFirst` the page it makes; real-input step 5 | — |
| A packaged build reading real browsers (Keychain prompt, Full Disk Access in System Settings) | not run: the brief forbids reading real browser data or granting Full Disk Access in a lane | — |
| Real-input rows ("Real-input batch steps" below) | not passed yet: deferred to the next real-input batch | — |

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

2026-10-09 (`prepare` and implementation, draft PR [#354](https://github.com/ccheever/exact2/pull/354)):

- **Profiles** (`browser-profiles.ts`): the reference's `browserProfile.ts` rules (Default and Incognito built in, up to 24
  named profiles of up to 48 characters, Incognito never the default), kept at the preference file's root
  (`browserProfiles`, `browserDefaultProfileId`). A tab opens only once the settings were read
  (`BrowserSettingsReadError`), at the configured profile and viewport.
- **Stores** (`T3BrowserSessions.swift`, `T3BrowserSessions+Profiles.swift`): Default and named profiles persist per
  environment (part 1's `storeIdentifier`); Incognito is `WKWebsiteDataStore.nonPersistent()`. Clear cookies and Clear
  cache (`browserClearData`) act on the tab's environment and profile; removing a profile clears its stores in every
  known environment.
- **Menus and badge**: the "+" menu's Browser submenu and the launcher's chevron list the profiles; the chrome row's
  badge shows a profile other than the configured default; the More menu's Profile group works. The menus take
  ↑/↓/Escape (KeyMenu).
- **Cookie import**: the reference's `BrowserImport/*` ported to the data module over the module's primitives
  (`browserImportIO`: SQLite, CommonCrypto/CryptoKit, Keychain, fcntl; `T3BrowserImportIO.swift`); writes through the
  profile's `WKHTTPCookieStore` with a read-back. Development and agent builds read only a fixture home
  (`T3_BROWSER_IMPORT_HOME`, confined); only the packaged build reads the user's browsers.
- **Settings**: Settings › Integrations › Browser's profiles row with its menus, the removal dialog and the import
  wizard (quit, Full Disk Access, configure, importing, done, blocked steps), and the defaults group.
- **Wired into parts 2 and 5** (after merging them): part 5's link opens (`openUrlInPreview`: chat, terminal and script
  links) and an agent's new tab open under the configured viewport and profile, and its host reads Auto-show; every page
  the module creates gets the default zoom and appearance through part 2's `browserSet` (on `browserSync`, whichever path
  opened it); Show device toolbar opens at a fixed configured viewport (`browserResponsiveViewportForToggle`).
- **Two fixes the live drives found**: the app runtime freezes `Error.prototype`, so `this.name = …` in an Error subclass
  throws there while Bun passes (the import's error classes now define `name`; a test runs the readers under a frozen
  prototype); a listing whose reads a newer command let go read every browser as running (the let-go answers are now
  discarded). Two test-id collisions (the wizard's dialog against its buttons, the defaults rows against their selects)
  made agent taps land on the container; both renamed.
- **Base merges**: `5f0ae7dca` (part 5, #346) and `46436acc3` (part 2, #348). Part 2 was reverted on the base by #351
  (`c603c22d6`); per the coordinator this branch keeps `46436acc3` and does not merge past it. Part 4 depends on part 2's
  re-land (its `browserSet` and `browser-viewport.ts`), and merges the base again once part 2 is back.

2026-10-10 (after part 2's re-land, #352):

- **Merges**: `e075de6e6` (the base at `ab220bfdb`: #351's revert, #352, #353, #355–#357, #359–#362, #364–#366) and
  `01da6c0d8` (the base at `a3bcac354`: #367–#370 and the records sync). Conflicts kept both sides: #355's
  `SurfaceLauncher` (its keyboard highlight) passes part 4's profiles to the Browser row's chevron; `browserSync`
  carries part 5's adopted notes and part 4's new-page defaults; Settings › Integrations keeps #353's reads beside
  part 4's Browser rows; STATUS took the base's part 2 and #353 rows.
- **The default zoom in `liveSessions`' zoom field** (`73aab89a0`): a page the module has not reported yet is made at
  the default zoom, so #352's `ensure` makes it at 487.5 × 1055 points and its first layout is 390 × 844 CSS px at
  125%; a reported zoom stays the tab's own; unread settings give 100%.
- **The launcher's chevron** (`0f09b7574`, `289861020`): with #355's launcher keys, the chevron's Enter reached the
  launcher, which opens its lit row. The chevron is now a keyboard menu trigger like part 4's other menus
  (KeyMenuOpen, KeyMenu: Enter and Space open the list at its first item, ↓ and ↑ at the first and the last), and its
  keys and its list's Enter stay out of the launcher (the reference's `handleKeyDown`).
- **Tests**: `browser-profiles.test.ts` resets the app's primary before each row (another file left one set, and the
  clear-in-every-environment row counted it).

2026-10-10 (the independent review's six should-fix rows):

- **Restore defaults** (`1c6ba8c7c`): `restoreLabels` ends with the reference's `getChangedBrowserSettingLabels` (Browser
  viewport, zoom, appearance, Recording frame rate, key presses, mouse presses, Open links in, Floating preview; the
  viewport compared by what it describes), and `restoreDeviceDefaults` resets the default viewport, zoom and appearance
  kept at the preference root (`restoreBrowserTabDefaults`) with the client settings. Its three tests are ported under
  their names (`browser-defaults.test.ts`); `settings-core.test.ts` restores with no environment connected.
- **The viewport menu** (`ade115849`): `BdViewportSelect` heads the presets "Standard" (SelectGroupLabel) and
  right-aligns each size in muted tabular figures; the popup is 278 wide at its trigger's right edge (align end), as
  measured in the reference. The view's options carry `detail` and `heading`.
- **Declared differences** (`ade115849`): the part-4 rows of `EXACT2-GAPS.md` that had no issue number now match the
  reference and are gone. The viewport menu, with its "no stepper arrows", which was no difference (the reference
  renders only `NumberFieldInput`). Full Disk Access, now read again by `app.contract`'s gated tasks `fdaPolling` (every
  1.5 s while the step shows and the window is visible) and `fdaFocused` (when the window takes the focus). The Browser
  section, which `IntegrationsPanel` now draws outside `data.available`, as `IntegrationsSettingsPanel` does. The first
  layout at the default zoom stays declared, under X1 path B (#100).
- **STATUS.md**: back to the base's; the coordinator syncs the real-input rows (steps 1–5 below, step 5 included).
- **The chevron's keys** (`1ce5f5dc7`): `browser-launcher-chevron.test.contract` replaces the source-text test in
  `r4-surfaces.test.ts`; it presses the keys through the app's handlers (`agent.mjs macos --test`).
- **Links** (`98949495d`): the old Fill/Default stub is gone. `openTerminalLinkInPreview` takes the configured defaults
  (`resolveBrowserOpenDefaults`) before the open and outside its fallback, and passes them to `openUrlInPreview`.
  `resolveLinkTargetPreference` refuses unread settings (`BrowserSettingsReadError`), so a chat or terminal link opens
  neither browser then, as the reference's `ensureClientSettingsHydrated` and `useOpenLink` do; the row's display keeps
  `linkTargetPreference`. In the app a command reads the settings before its op runs, so this guards the early-start
  path only.
- **Merges**: `eb8daccda` (#372–#374; `settings-source-control.contract` kept #374's writer-model picker and part 4's
  Browser section) and `657ca0794` (#371).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Unit, AppKit and the five checks | `827470bbc` | Bun 3,934 pass / 0 fail / 1 skip (part-4 files: profiles 29, import 30, readers 30, Safari 25, sources 43, defaults 6); strict tsc clean; contract build OK (5,966 slots); `cargo test -p t3-code-macos --lib` 13; AppKit `browser` 27, `browser-profiles` 16, `browser-automation` 20, `menus` 45; build, test (3,521 passed, 0 failed, 34 ignored), clippy, fmt, caps, boot: all exit 0 | [checks](https://raw.githubusercontent.com/ccheever/exact2/f0013459ca171ace31503b60a90fb8968d3af3a5/browser-surface-profiles/checks-827470bbc.txt) | — |
| Live drives 1–3 (agent mode) | work in progress (before `4e4f76f23`) | 1: the agent's `type` replaces a field's text, so the rename op was rewritten. 2 and 3: the Chrome wizard opened on "Quit Chrome": the readers threw `TypeError` in the app runtime (a frozen `Error.prototype`; Bun passes), and a let-go listing read every browser as running. Both fixed in `4e4f76f23` with tests | — | — |
| Live drive 4 (agent mode) | `c7a0aa02f` | Ops 1–102: add, rename, remove; Chrome import (3 imported, 2 skipped); Brave's quit step; Safari's Full Disk Access step; Arc blocked; Firefox into Default. Done and "I've quit it" missed their buttons: the dialog's test id was the button's (`browser-import-<step>`); fixed in `0af00d295` | [record](https://raw.githubusercontent.com/ccheever/exact2/2190ae19fcd99108474fc144dac23f7d69919621/browser-surface-profiles/drive-branch-4.txt) | — |
| Live drive 5 (agent mode) | `827470bbc` (parts 2 and 5 merged) | Ops 1–102 passed, including Done and "I've quit it" → configure; stopped at op 103: each defaults row's test id was its select's, so the select did not open; fixed in `5a7522e56` | [record](https://raw.githubusercontent.com/ccheever/exact2/596efb792211934bb2ce71b68b19cd8536a2a8cd/browser-surface-profiles/drive-branch-5.txt) | — |
| Live drive 6 (agent mode, final mode: the Chrome and Firefox imports, the defaults rows, the tabs) | `5a7522e56` | All 117 ops passed: Chrome import into a new profile and Firefox into Default; the defaults rows set and reset; the launcher chevron (Default, Incognito, Chrome); a Chrome tab at the configured iPhone 12 Pro, 125% and dark with its imported cookies (the probe names `lane_chrome, lane_session`); the More menu's Profile: Chrome group, Clear cookies then none; "+" › Browser › Incognito keeps `incognito_probe` to itself; a Default tab sees `lane_firefox` only | [record](https://raw.githubusercontent.com/ccheever/exact2/ab29fa36a2bbad9660d6040ce286e58e1fbe2c20/browser-surface-profiles/drive-branch-6.txt) | — |
| Before drive (agent mode, the first-layout steps) | `e075de6e6` (the merge alone) | All 49 ops passed; the fixture's first layout 312 × 675 at 2.5, then 389 × 844 | [record](https://raw.githubusercontent.com/ccheever/exact2/ef2b2b5afbc7482c94f7d0cc04913e46d6251cf2/browser-surface-profiles/drive-before-e075de6e6.txt) | — |
| Live drive 7 (agent mode, full) | `01da6c0d8` | All 165 ops passed: the chevron's Enter, the first layout at 125% (389 × 844 at 2.5, no resize), part 4's rows again | [record](https://raw.githubusercontent.com/ccheever/exact2/360f68695daa976dc1164c818774cab1026dee67/browser-surface-profiles/drive-after-01da6c0d8.txt) | The Bun suite then failed twice: `menu-keys.test.ts` held the chevron's new `key=` to the keyboard-menu rule, and `browser-profiles.test.ts` counted a primary another file left set; both fixed in `289861020` |
| Live drive 8 (agent mode, full; the one retry) | `289861020` | Ops 1–135 passed: the chevron's Enter, ↑ and Escape, the first layout (389 × 844 at 2.5, no resize), both imports, the defaults rows, "+" › Browser › Chrome. Op 136 failed in the driver: "clock: the clock cannot go backwards (106800.0 → 106799.99999999999)" | [record](https://raw.githubusercontent.com/ccheever/exact2/b640b82a1281dcd936fa76e0271899de0d133259/browser-surface-profiles/drive-after-289861020.txt) | The driver's clock arithmetic (`scripts/agent.mjs`), not the app; ops 136–174 passed at `01da6c0d8`, which differs only by the chevron's menu |
| Unit, AppKit and the five checks | `289861020` | see [checks](https://raw.githubusercontent.com/ccheever/exact2/19bb5e5f52b437c3b03e6d882977f8dde84fc2d3/browser-surface-profiles/checks-289861020.txt) | [checks](https://raw.githubusercontent.com/ccheever/exact2/19bb5e5f52b437c3b03e6d882977f8dde84fc2d3/browser-surface-profiles/checks-289861020.txt) | — |
| Live drive 9 (agent mode, full; the review fixes) | `eb8daccda` | All 195 ops passed: every row of Results with its before (the evidence base `950e8e2e5`, 41 ops) and reference (CDP on the same fixture stores); the Full Disk Access step's `fdaPoll` 2 → 4 in 3.2 s; Restore defaults' confirmation and reset; the probe got `lane_chrome, lane_session`, none after Clear cookies, `incognito_probe` in Incognito, `lane_firefox` in Default. The offline session (no embedded server) drew the Browser section | [after](https://raw.githubusercontent.com/ccheever/exact2/78b9d29a596ae182e056f5996b343ddf670cc4b7/browser-surface-profiles/drive-after-eb8daccda.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/1b75884cfe6e37498b19d2367b7d207264187453/browser-surface-profiles/drive-before-950e8e2e5.txt), [offline after](https://raw.githubusercontent.com/ccheever/exact2/ef8f66027516918f551ebc1ed83000d062ddd314/browser-surface-profiles/drive-after-offline.txt), [offline before](https://raw.githubusercontent.com/ccheever/exact2/c87a96b71d7b65f40dfd852e18860c2ed5f0e0ca/browser-surface-profiles/drive-before-offline.txt), [drive3.sh](https://raw.githubusercontent.com/ccheever/exact2/bef461359ed8b7d9d77bdab23b015799332e0588/browser-surface-profiles/drive3.sh.txt), [compose3.py](https://raw.githubusercontent.com/ccheever/exact2/fb3db923f901992a75c0ecfcd822910e18f15f00/browser-surface-profiles/compose3.py.txt) | — |
| The chevron's Contract test (agent mode) | `eb8daccda`, and the same build with the chevron's key guards taken out | 3 of 3 passed; unguarded, test 1 failed at lines 27–28 (the launcher opened Files) | [runs](https://raw.githubusercontent.com/ccheever/exact2/38dcd0b86ccc10246b028c0fe92742f514fa33d1/browser-surface-profiles/test-chevron-contract.txt), [drive-test.sh](https://raw.githubusercontent.com/ccheever/exact2/8f2b93ee3ee8a21633d6eada1e9f46ac4eb21049/browser-surface-profiles/drive-test.sh.txt) | — |
| Bun, tsc, contract build, `t3-code-macos` lib, AppKit, caps and the five checks | `657ca0794` | Bun 4,156 pass / 0 fail / 1 skip (285 files); strict tsc clean; contract build OK (6,159 slots); `cargo test -p t3-code-macos --lib` 16; AppKit `browser` 29, `browser-profiles` 16, `browser-automation` 25; caps within budget; build, test (3,521 passed, 0 failed, 34 ignored), clippy, fmt, boot: all exit 0 | [checks](https://raw.githubusercontent.com/ccheever/exact2/eb183dcd7f989d5b289a4d21b2b31f7afa046e36/browser-surface-profiles/checks-657ca0794.txt) | — |

## Real-input batch steps

The lane: `target/lane` in the profiles worktree (`drive.sh <run> final` is the agent-mode version; fixture home from
`make-browser-home.mjs`, probe page `fixture.mjs` on 16781, server on 16780; isolated HOME, CODEX_HOME,
CLAUDE_CONFIG_DIR, XDG_*, T3_LOCAL_HOME). A lane app copy needs its own name and bundle id; never the user's browsers. Copies: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/ee83a8e71fc5ffbd2157318707b7818c8a8a9849/browser-surface-profiles/drive.sh.txt), [make-browser-home.mjs](https://raw.githubusercontent.com/ccheever/exact2/0f4b4d0ca3b5cc40792b8663566aac4a74b4fe3a/browser-surface-profiles/make-browser-home.mjs.txt), [fixture.mjs](https://raw.githubusercontent.com/ccheever/exact2/a634c7f48ebe1a77f70bac9604144a433c0a8137/browser-surface-profiles/fixture.mjs.txt).

1. **Stores across a relaunch** (needs a normal launch: agent runs keep every store in memory). Import Chrome into a new
   profile, open a Chrome tab at `127.0.0.1:16781/cookies` (names `lane_chrome`, `lane_session`), open an Incognito tab at
   `/set`. Quit with ⌘Q and launch again normally. Read back: the Chrome tab's probe still names `lane_chrome`; a new
   Incognito tab's probe names none.
2. **The profile menus by real keys.** Settings › Integrations › Browser: focus Add profile, press ↓: the menu opens at
   Blank profile; ↓ reaches the first browser under "Import from"; Escape closes it and the focus is back on Add
   profile. The same on a row's ⋮ menu and on the launcher's chevron. On the launcher (Toggle right panel on a thread
   with no surface open): press ↓ three times (Files lit), Tab to the Browser row's chevron, press Return: the list
   opens at Default and no surface opens; Escape, then ↑ on the chevron: the list opens at Incognito; Escape; the
   launcher still lights Files.
3. **Real pointer.** Hover a tab's profile badge: the tooltip names the profile. "+" › Browser: the chevron opens the
   profile list (X66: by press, not on hover).
4. **Rename by real key events** (ASCII; the input source is left as it is): type in a row's name field and press
   Return; the "+" menu shows the new name.
5. **A tab made with its URL at 125%** (a real click; the lane's `drive2.sh` environment and `fixture-first.mjs` on
   16782, [copy](https://raw.githubusercontent.com/ccheever/exact2/95682cec2b9dfdd2452804fb97231652fb2f822c/browser-surface-profiles/fixture-first.mjs.txt)).
   Settings › Integrations › Browser: Default browser viewport = iPhone 12 Pro, Default browser zoom = 125%, Open links
   in = T3 Code. Open the terminal drawer on a thread, run `echo http://127.0.0.1:16782/`, and click the printed link.
   Read back: a Browser tab opens beside the thread at iPhone 12 Pro and 125%, and the fixture's log has
   `/first?w=389` or `w=390`, `h=844`, `d=2.5` for that load and no `/resize` after it.

## Next action

1. The real-input batch (steps 1–5 above). Step 2's launcher chevron is also `browser-launcher-chevron.test.contract` in
   agent mode; the real keys stay a batch row.

## Delivery

Merged on 2026-10-10 as `a6e31eed7` (#354, squash) after an independent review and its repair round. Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
