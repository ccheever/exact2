---
name: 20261008-fix-provider-auth-state
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-fix-provider-auth-state'
pr_url: null
verified_commit: null
---

# Provider sign-in state stays current

## Outcome

Five clone bugs from the real-input batch ([#298](https://github.com/ccheever/exact2/pull/298), its
numbering) in the provider sign-in surfaces, fixed as the reference T3 Code (`1e2ecbd975`) behaves:

- **17** ([#256](https://github.com/ccheever/exact2/pull/256)): after Disconnect the saved account row showed the email in clear.
- **18** (#256): right after a ChatGPT sign-in the app re-subscribed to `provider.auth.subscribe` about 224 times in 20 s.
- **19** (#256): after Reconnect the Settings › Providers list row stayed "Not authenticated" while the editor said authenticated.
- **20** (#256): with Settings › Providers open the app called `server.refreshProviders` every 11 s.
- **21** ([#238](https://github.com/ccheever/exact2/pull/238) follow-up): the Add provider dialog's Sign-in method select did not open by click, Space, Return or ↓.

Plus a coordinator row (user decision 2026-10-08): the managed Codex "hand a ChatGPT sign-in to
another machine" check that #256 left open, on a LAN address.

## Scope and exclusions

Included: the cause of each bug, a fix that matches the reference, a regression test that fails on
the base, a live row, before/after evidence. Excluded: framework edits (X64 is reported, not
changed); Escape in the open method menu (it also closes the Add provider dialog) and ↓/↑ to open the
menu: both belong to fix-keyboard-focus (`KeyMenu modal=true` through the `CnMenu` kit, the shared ↓/↑
pattern); keyboard movement inside a menu (#298 bug 13's area).

## Cause and fix

| Bug | Cause | Fix (reference) |
| --- | --- | --- |
| 17 | Not a clone bug. The reference draws the disconnected row's saved account email in plain text: `(reconnectEmail ?? "Use your ChatGPT subscription.")` (`apps/web/src/components/settings/CodexSetupSection.tsx:604`), as it draws "Continue as ${requestedAccountEmail} on OpenAI." (`:583`). `RedactedSensitiveText` (`RedactedSensitiveText.tsx:27-61`) reads no redaction setting, and the reference applies it only to "Signed in as" (`:589-597`). The clone already did the same | No change (the user's rule: behaviour matches the original). A `RedactedText` version was built and reverted before the commit |
| 18 | `welcomeView` (`pages-welcome.ts`) let go of every welcome stream when the step was not "agents" and then mounted the Codex rows again on the projects step: each answer unsubscribed and resubscribed both streams, and each subscription's first event asked for the next answer | The Codex setups are mounted on the agents step only, as `WelcomeWizard.tsx:269-281` mounts `AgentsStep` |
| 19 | Framework, macOS (**X64**): a wrapped paragraph drawn from a text raster that shrinks below the raster size (16,384 device pixels) keeps painting its old raster. The row's status went from two lines ("Not authenticated · Sign in with ChatGPT to use Codex.") to one ("Authenticated · ChatGPT"); the tree had the new text | The status text is a new node per status (`each status in [row.status] key=status`, `providers.contract`); the editor's status line the same, keyed by its parts (after Disconnect its detail kept its words, moved up beside a shorter lead and its old raster ran under the name field). Workaround for X64 |
| 20 | The root's `liveTick` kept ChatComposer's workspace discovery running behind Settings (and the Usage / Pull Requests pages and the welcome): a pending snapshot re-armed `server.refreshProviders {instanceId, cwd}` every 10 s + the 1 s tick | `composerWorkspace(data.revision, not settingsOpen and not pageCover and not welcome.show)`: the reference mounts ChatComposer on the chat view only (settings, usage and welcome are routes of their own), and mounting it again starts its refs over (`WorkspaceDiscovery.state(…, shown)` drops its attempts when shown again). `app.contract`: one line edited, 0 added |
| 21 | After "Continue to sign-in" the page behind the dialog selects the created instance (`providerSelected = providerCreated`) and draws the same Account row: the dialog's `popovertarget` named the menu of the row behind it (duplicate ids) | `ProviderAccountRow` takes `idPrefix` ("wizard-" in the dialog) for its menu, items, combobox, Sign out and other test ids, and `ProviderWizardAuthStep`'s fallback row prefixes its test ids; the dialog's Sign out cancel focuses `wizard-provider-sign-out-<id>`. Click, Space and Return open the menu. ↓/↑ (Base UI's Select trigger): moved to fix-keyboard-focus (the shared menu pattern; see "Hand-off" below) |
| (handoff row) | The clone refused remote http origins ("A remote server requires HTTPS.", `T3Protocol.swift` `T3Endpoint.origin`); the reference accepts http and https remote backends (`packages/shared/src/remote.ts:6`, `SUPPORTED_REMOTE_BACKEND_PROTOCOLS`) | `T3Endpoint.origin` takes http as the reference does (the transport XCTest now expects it) |

## Acceptance

Lanes (not committed): `target/lane/fpas` in this worktree. Agent drives at 1280×840 with `env -i` and
isolated HOME / CFFIXED_USER_HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, TMPDIR and T3 homes per lane,
embedded server on lane ports 16250-16262, the T3 runtime 0.0.46-nightly.20261005.2667. Before = the
evidence base at `c0475fbaa` (built under its `.build-lock`, then copied out and driven by path).
Real-input sessions: a lane copy "T3 Code (Lane FPAS)" (`com.exact.t3code.macos.lanefpas`, ad-hoc
signed) launched by LaunchServices, under the shared real-input lock (07:27-07:40Z and 08:31-08:53Z).

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| 17 disconnected row email | not a clone bug; no change | the reference lines in the table above; live Disconnect 07:34:52Z (one `provider.auth.logout`) in [live-session](https://raw.githubusercontent.com/ccheever/exact2/1f6a87675f220f9bc4dbd3d55958b3a01e721036/fix-provider-auth-state/live-session.txt) | — |
| 18 subscribe burst | pass | [e18-subscribe](https://raw.githubusercontent.com/ccheever/exact2/d2eb14197c050b23237e392d18458c32e075df3d/fix-provider-auth-state/e18-subscribe.txt): base 1,275 `provider.auth.subscribe` (+1,275 install) on the Projects step in 20.6 s; branch 1 + 1, none on Projects; after a real ChatGPT sign-in, none on Projects (07:32:44-07:33:14Z). Test `pages-welcome.test.ts` "the agents step holds one sign-in subscription per stream, and the projects step holds none" (base: 6 extra subscribes) | — |
| 19 stale list row | pass (X64 workaround) | [01 pair](https://raw.githubusercontent.com/ccheever/exact2/57f1c450a7bb24f0b9ea7c1e26de79f8c37068d1/fix-provider-auth-state/01-list-row-disabled.png): Codex switched off, tree "Disabled" in both, base paints the old "Not auth…" raster; [08 live](https://raw.githubusercontent.com/ccheever/exact2/9a8bc33434736bf01c87b0d233a087bfc9a7336c/fix-provider-auth-state/08-reconnect-list-row-live.png): right after Reconnect (real consent) the row reads "Authenticated · ChatGPT"; X64 one-file app [06](https://raw.githubusercontent.com/ccheever/exact2/0574dbb319f94727f96392c60d55e220e77a646f/fix-provider-auth-state/06-x64-one-file-app.png), on main [06b](https://raw.githubusercontent.com/ccheever/exact2/d385ab70b347b1bed6e04a67227b7f2514c2d292/fix-provider-auth-state/06b-x64-on-main.png). Test `providers.test.ts` "the list row and the editor read the same status…" (a source guard: X64 is pixels; the drives are the proof) | X64 (framework) |
| 20 refresh loop | pass | [e20-refresh](https://raw.githubusercontent.com/ccheever/exact2/e3d655923fbb2575d48ec122a9caba8a7d80b0d5/fix-provider-auth-state/e20-refresh.txt): Settings › Providers open 75 s with Codex signed in and the demo draft on Codex: 0 calls (6 min in session 1: 0); base every 11 s (#298). Test `composer-workspace-snapshots.test.ts` "an unmounted composer asks for nothing…" fails on the base | — |
| 21 wizard method select | pass for click, Space, Return; ↓/↑ moved to fix-keyboard-focus | [02 pair](https://raw.githubusercontent.com/ccheever/exact2/270ae2d0072ece4285830dbf45b629acab445a2b/fix-provider-auth-state/02-wizard-method-click.png) (agent platform click: base no menu, branch menu); [05](https://raw.githubusercontent.com/ccheever/exact2/713e4061bc7b80bc8636e24171665a865bc9c2ec/fix-provider-auth-state/05-wizard-method-keys-after.png) (agent keys on the focused trigger, pick); [10](https://raw.githubusercontent.com/ccheever/exact2/3e597c482903b352da16abc4ff30c3694db87ec9/fix-provider-auth-state/10-wizard-method-real-input.png) real click opens it (session 2). Real Space/Return after the click and after Tabs: no menu, but no focus ring is drawn, so where the focus sits is not visible (fix-keyboard-focus's rings). Test `provider-setup.test.ts` "the Sign in step and the Settings editor behind it draw the same Account row under ids of their own" | ↓/↑: fix-keyboard-focus |
| Hand a ChatGPT sign-in to another machine (#256 row 7; user decision 2026-10-08) | pass | [07 before](https://raw.githubusercontent.com/ccheever/exact2/6e2d0548f06806536e445ba37ac9aea6cfc33be5/fix-provider-auth-state/07-lan-http-refused-before.png): the base refuses the LAN link ("A remote server requires HTTPS."); [09 after](https://raw.githubusercontent.com/ccheever/exact2/c6da84ac4b12eceb6bf557f521bf673c9fee43d6/fix-provider-auth-state/09-lan-handoff-after.png): "Your ChatGPT plan is connected · Codex on Daehyeon's MacBook Pro" and the LAN environment's Codex row "Signed in as" + placeholder, Ready; [live-session-2](https://raw.githubusercontent.com/ccheever/exact2/e2cf48aef331485fb79ee338405ebf5e0467cf87/fix-provider-auth-state/live-session-2.txt): LAN server pid 31264 on 192.168.1.225:16261 only, paired over http (a lane keychain; the real search list unchanged), the remote's Codex installed, the primary's `CodexChatGptAuth.exchange` Success 08:43:57Z, the remote's `caches/codex.json` ready / authenticated chatgpt, no `code=` in either server's logs or the drive, 5 of 5 lane client sessions revoked (`t3 auth session list`: none), the server stopped, nothing listening on 16261 | — |
| Real sign-in and Reconnect (consents) | pass | [04](https://raw.githubusercontent.com/ccheever/exact2/3448441e45580b1de8d79dcd89f6ed4b44d3de9a/fix-provider-auth-state/04-welcome-signed-in-live.png): first sign-in (07:31:52Z consent by the agent's click, `provider.auth.complete` Success 07:32:00Z, the app to the front); Reconnect consent 08:34:28Z → `auth.complete`; the handoff's OpenAI page returned to the loopback callback without a consent page (granted minutes before) | — |
| Visual oracle and trace | not run | — | user decision 2026-10-06 |

Seen on the way:
- The lane copy asks for the Documents folder at every launch ("T3 Code (Lane FPAS)" would like to access
  files in your Documents folder; reported by the coordinator at screen points 546,160). Answered Don't
  Allow twice (08:31Z and 08:50Z, the second after a `tccutil reset SystemPolicyDocumentsFolder
  com.exact.t3code.macos.lanefpas`); gone both times (window list, `screencapture`). The agent-mode builds
  (launched from a terminal) never prompt. Trigger not pinned: the lane server inherits LaunchServices'
  PATH, which on this Mac is the user's full PATH (`~/.local/bin`, `~/.bun/bin`, mise / asdf / volta
  shims …), so the server reached the user's own `claude` 2.1.293 (shown as "Claude v2.1.293") and could
  reach `~/.local/bin/codex`. Next time pass `--env PATH=/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin`
  to `open` (the login-shell probe merges the app's PATH).
- A lane HOME without a keychain cannot store a remote's credential ("Keychain Not Found" from
  SecurityAgent; Cancel answered). A lane keychain made with `HOME=<lane> CFFIXED_USER_HOME=<lane>
  security create-keychain` keeps the user's search list unchanged; deleted afterwards.
- Settings › Providers' environment menu (on <environment>) checked the LAN environment but the page kept
  showing the primary's providers (normal launch, 08:41Z); the welcome's agents step was used for the
  remote instead. Not investigated.
- Reconnect ChatGPT's picker lists the saved profile by its name, which is the email ("… · Connection 1"),
  in plain text, as the reference does (`ChatGptAccountPicker.tsx:59`).

## Real-input batch steps

None left for this task. Real Space and Return on the dialog's method select need a visible focus
ring (fix-keyboard-focus) to be read back; agent-mode keys on the focused trigger pass.

### Hand-off: ↓/↑ opens a menu (to fix-keyboard-focus)

Contract has no `showPopover(id)` action (X66), so a key handler cannot open a `popover`. What worked
here (built, driven in agent mode on this branch's build 9 with ↓ and ↑ opening the method menu below its
trigger and a later tap still picking an item, then removed from this PR so the shared pattern lands once):

```contract
  state methodFocus = false
  action focusMethod(on: bool)
    methodFocus = on
  …
  button popovertarget=menuId role="combobox" focus=focusMethod(true) blur=focusMethod(false) …   // the trigger
  // A second invoker over the trigger, invisible and click-through, that shows the menu by ↓/↑ only
  // while the trigger has the focus:
  button popovertarget=menuId popovertargetaction="show" aria-keyshortcuts=(methodFocus and not off ? "ArrowDown ArrowUp" : "") aria-hidden=true tabindex=-1 pointer-events="none" position="absolute" left=0 top=0 width="100%" height="100%" padding=0 border-width=0 opacity=0
```

Findings from the one-file probe (`target/fpas/x62app/.exact/app-x63probe.contract`, not committed):
- the invoker must stay mounted: one inside `when methodFocus` unmounted on blur, and a menu whose
  invoker unmounts closes before its item's press lands;
- a 1 pt invoker anchors the menu at the trigger's top-left (it overlaps the trigger); full size with
  `pointer-events="none"` anchors it as the trigger does and lets clicks through;
- ↓ again on an open menu keeps it open (`show` does not toggle); with the focus elsewhere ↓ does nothing;
- risks the review raised: if the trigger unmounts while focused and `blur` does not fire, the shortcut
  stays armed on remount; ↓/↑ on an open menu are swallowed (no in-menu movement yet, #298 bug 13).

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| agent drives, before/after | base `c0475fbaa` / branch builds 3-9 | 18: base 1,275 subscribes on Projects, branch 1; 19: base stale "Not auth", branch "Disabled"; 20: no trigger without a signed-in Codex (0 / 0); 21: base click opens nothing, branch opens; keys open on the branch | e18, 01, 02, 05 | — |
| one-file app (X64) | framework of `c0475fbaa`, then main `475043d20` | `line-clamp=2` and plain wrapped texts keep the old raster after shrinking to one small line; the keyed, the one-line and the wide (rastered) texts update; the capture path is right | 06, 06b | X64 |
| live session 1 (real input) | build 6 | first real sign-in (consent clicked by the agent), welcome Projects without re-subscribes, Disconnect; Reconnect not reached: the screen locked at 07:40Z; LAN pairing refused by the http rule | live-session, 04, 07 | — |
| independent review | the uncommitted diff (build 7) | no blocking findings. Taken: whitespace and reveal carry-over notes (moot after reverting 17), the remaining account-row test ids prefixed, the ↓/↑ caveats recorded. Kept with reasons: the http change (matches the reference; needed by the user's LAN row). Not taken: the Add ChatGPT account dialog's `codex-account-<id>` ids may also collide with Settings' (not verified; not this task's bug) | — | — |
| agent handoff | build 8 (http rule) | LAN pairing and the remote's install + handoff start in agent mode; the consent then ran in Chrome on the recorded URL | live-session-2 | — |
| live session 2 (real input) | build 9 | Reconnect (real consent): list row follows; Settings open 75 s: 0 refreshes; LAN pairing in the .app (lane keychain); the handoff completed (agent mode + Chrome); real click on the wizard select | live-session-2, 08, 09, 10, e20 | — |
| coordinator review | after build 9 | 17 reverted to the reference; ↓/↑ invoker removed (moved to fix-keyboard-focus); Excluded line; X64 re-run on main | — | — |
| checks | merged tree | `bun test examples/t3-code` 3139 pass / 1 skip / 0 fail on the merged tree (runner attempt 1 before the merge: 3053 / 1 / 0); strict tsc clean; contract build 3860 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11 pass; AppKit transport 57 / 0, fleet 9 / 0, codex-auth 6 / 0; caps within; five checks on the merged tree: build exit 0, test 3,521 passed / 0 failed / 34 ignored (94 binaries), clippy and fmt clean, boot allowed paths only; verify runner passed, `source_unchanged: true`, the committed tree matches | `target/fpas/verify/attempt-1`, `target/fpas/checks` (not committed) | — |

## Progress

2026-10-08: implemented, verified (runner attempt 1 passed, `source_unchanged: true`, committed tree matches;
independent review without blocking findings) and pushed as `fe6629054` with the merge of
`732f0e3f3`; draft PR against `feat(example)/t3-code`.

## Next action

Coordinator: review and merge the draft PR. X64 waits for the user's approval to publish. ↓/↑ on the
method select arrives with fix-keyboard-focus's shared menu pattern.
