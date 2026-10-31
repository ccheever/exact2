# T3 Code clone handoff

Updated October 30, 2026, Toronto time. Finish the one-to-one clone with pstack
handling routine execution and review. The legacy scheduled coordinator stays
disabled. Use this handoff when taking over the work.

## Current state

The clone is incomplete. The desktop app is in `examples/t3-code`; the separate
iPhone and iPad app is in `examples/t3-code/mobile`.

The current priority is iOS. Desktop feature work is deferred. Continue in this
order, with small verified commits and normal pushes to PR216:

1. Pair the normal iPhone app over the verified existing HTTPS route below.
2. Open a project, create a thread, send, observe streaming, Stop, then restart
   and recover the environment, transcript and unsent draft.
3. Verify reconnect, offline recovery, retries and duplicate-send prevention.
4. Complete Photos and Files selection, cancellation, preview, removal and Undo.
5. Verify keyboard focus, Return behavior, IME, hardware shortcuts and voice.
6. Complete connected file previews, Markdown, selection, Copy and cache recovery.
7. Compare iPhone navigation, layout, accessibility and performance with the pin.
8. Deliver the phone checkpoint as a signed production build through TestFlight.
9. Complete iPad layout and the same working flows, then deliver its TestFlight build.

Home and thread-detail workers now also target current upstream mobile UI,
including iPhone Duo improvements. The original repository is cloned locally at
`~/Projects/t3-code`, with clean main snapshot
`80a2351fcb909034e65a512dfabeb3cdfd367762`. Record and share the chosen reference
SHA before comparisons; use one fixed snapshot per acceptance and benchmark run.
The existing vendored mobile source pin remains `365aa87982` until its changes
are adopted and qualified separately. It is no longer the sole UI oracle.

Pixel parity includes container padding and margins, overflow and clipping,
native headers, safe areas and all interactive states. Qualify the actual iPhone
Duo simulator folded and unfolded, in portrait and landscape, through fold,
rotation, keyboard and sidebar transitions. Preserve the reading position and
draft. Simulator-agent swarms are authorized for separate interaction cases,
with one owned device/session and isolated writable work per worker. Performance
comparisons run without competing swarm load. Coordinate shared files and merge
order through this thread; each worker owns its feature branch and PR.

The existing HTTPS origin is `https://prod-c4cb6b5152362498.t3coderelay.com`.
It was found in current server logs and verified with system TLS validation.
Its public descriptor returns HTTP 200 for MacBook Pro,
`fb57350b-ce98-48b3-a812-df134e42156c`, protocol 2, server
`0.1.6-nightly.20261030.3917`. The fresh EAS iPhone also loads that descriptor
in Safari and displays the same identity. This proves network reachability,
not app authentication. No new tunnel, exposure or credential was created.
Normal app pairing and connected acceptance remain next. Route and normal-app
evidence are under `target/t3-poc-evidence/ios-focus-20261031/`.

The requested production release `1.0.0 (20261031003812)` is available in internal
TestFlight. Its clean source is `358dce46d1defe3260c401cde377b7dd8aefa4f9`, including
Exact main `43e0d93b85` and the prior native NewTask and preview corrections.
The exact signed IPA was submitted through
[EAS](https://expo.dev/accounts/exponathan/projects/t3-code/submissions/22b77f7f-db38-48ca-85e0-d4c33e898c13).
Its SHA-256 is `62ce7464d0e147fedc7272603718938d4b06bda37ca53170fa46cdfdb57c3132`.

EAS reports FINISHED. Apple build `93e6a664-3609-4cfc-82a2-7c6b792216e0` is VALID,
IN_BETA_TESTING internally and unexpired. The existing Team (Expo) group explicitly
includes this build and retains its two testers. The accepted tester has group
access; the other existing invitation remains pending. Installation of this exact
build is unverified. The existing encryption declaration is accepted as false.
No external beta review or public release was requested, and no new invitation
was sent. No human-only release action remains.

Release evidence is under `target/t3-poc-evidence/testflight-20261031003812/`;
the archive and its build receipt are in the isolated `t3-f9c706fa` worktree at
`target/t3-testflight-release-20261031003812/`. This requested release does not
complete phone or iPad parity. Historical notes below about the older POC build
refer to `20261029015732` unless another build is named.

Continue on `t3/t3-clone-pr216-poc`, which pushes to PR216's remote branch
`feat(example)/t3-code-ios`. Keep PR216 draft and unmerged. The integration
commit is `85d466e2e3c5ff0c4e94b7b6065c6a91b3584609`, a normal merge of
`9db6b0d68ccf7d5780b46022ae16dbe2f4132d36` into
`fe242e5a8dfb557510fbe6bac09d4074c2ceeeb3`.

Desktop now follows reference `6d450a4284852af137b2f3407fddd89eb76e7644`
and the official stable `0.1.5` server. Mobile remains independently pinned to
`365aa87982`. The delivered TestFlight build above predates this desktop base;
this integration creates no release checkpoint. Exact main remains
`43e0d93b858143f85c93c729057fd601d0f8fba2`, already merged in `358dce46d1`.
C ABI 18, TypeScript runtime 7 and native ABI 3 remain unchanged.

Root has one active layout: the five current members plus the incoming
server-browser member. This keeps Root at 1,474 lines while preserving the
incoming declarations and their order. The prerequisite's owner-validating
`appText` reader is unchanged. Its tests read the active members; the fixture
retains existing interpreter semantics alongside incoming nested calls, sent
notifications, derives and attributes. Compiled-plan fixtures now use the
Permissions menu and each thread row's snooze presets.

The incoming uploads, permission and effort controls, accessible chip faces,
text direction, protocol operations, file viewers, navigation, settings and
Highlights producers are integrated. Shared generators retain mobile's exact
Shiki/parser dependency entries, typography and Markdown behavior while adding
Highlights and viewer packages. All tracked mobile files and ExactKit remain
unchanged. The only changed ignored mobile output is the shared Markdown bundle,
which also contains desktop minimap thread-link handling; its file parser is
unchanged. Browser draft signing, native NewTask ownership, finite raster
coverage, the Mac titlebar press fix and existing trust guards remain intact.

All six root check categories pass, including Clippy and formatting, as do
115 locked clone Rust tests, clone Clippy/formatting, 7,926 desktop tests,
4,170 mobile tests, strict desktop/mobile application TypeScript, 527 generator
outputs, eight generator tests and 201 selected native tests. Two existing
desktop heap-stress tests remain skipped. Root and clone Cargo checks use the
documented command-local `CARGO_INCREMENTAL=0`; the original incremental
identity failure is not repaired. Standalone fixture strict typing still lacks
Node/Bun declarations and is not green.

The clean normal Mac bundle from `85d466e2e3` verifies 512 files, 3,942 inputs,
its strict signature and the actual native factory table. Its bundled official
0.1.5 archive matches the pinned checksum. Through the supported Exact Mac
controller with `context.agent=true`, a fresh isolated server connects with
protocol 2 and `serverResolvedCommandContext`. Onboarding skips import and the
real palette adds one empty owned scratch project. The original panel header
opens, closes and reopens its chooser; thread details open and dismiss, and body
Add Project opens its palette. Six contacts each have one intended press and a
complete zero-drop journal; seven native captures were inspected. No conversation
is created and the composer stays empty. Normal Quit stops the app and backend
before controller close; all owned processes exit, the isolated port is free,
and the original listener and clone window defaults remain unchanged.

The clean normal iPhone no-install bundle verifies 92 files, 3,289 inputs and its
strict signature. It now installs and launches on a fresh EAS iPhone. Home and
Settings work. Changing the Return preference to Insert new line survives an app
relaunch; restoring Send message survives another relaunch. AX selection and an
inspected native capture agree. Hardware Return dispatch remains unqualified.
The owned EAS session is shut down, and the original T3 view is restored after
read-only route inspection. The delivered TestFlight record remains unchanged.
Unit 3 must qualify live
viewer/Copy/disposal and upload ownership flows, Browser A→B/Back/Forward/Reload,
Markdown and retention, provider flows, and phone/iPad parity. Ordinary Mac mouse
input, physical dragging and normal-app Keychain restart also remain unqualified.
Evidence and the complete local receipt are under
`target/t3-poc-evidence/pr216-base-9db6-integration/`. Independent review finds
no introduced P1 or P2 issue. Its focused desktop/mobile checks, 115 locked Rust
tests, generator and type checks pass. It verifies all conflict resolutions,
mobile preservation, signed artifacts, bundled runtime and raw native evidence.
Three redundant fixture comments are removed; 17 palette and nested-action tests
pass again. Unit 3 live qualification remains pending.

The unchanged 0.1.5 Mac artifact also passes fresh project-draft recovery through
that controller. Onboarding skips import, and the real palette adds one owned
scratch Git project. Its exact 75-character unsent draft, including Unicode,
survives normal Quit and a new-seed launch of the same named store and backend
home. Explicit project reopening retains the draft in app data and the live
composer. Inspected native captures agree. The first app and backend exit before
controller close, and the isolated port is free. This qualifies named agent
storage with `context.agent=true`; normal-app Keychain persistence stays open.
Evidence is under `target/t3-poc-evidence/desktop-015-provider-acceptance/`.
The same isolated 0.1.5 app also returns a real Codex reply. A later assistant
message grows from 726 to 2,474 characters while active. Actual Stop ends it with
Run interrupted. Extended stability and transcript restart remain untested.
The desktop session and all owned processes are closed. Further desktop work is
deferred for the iOS priority above.

## Earlier checkpoints

The desktop manifest's unused `t3-browser` native tag is removed in `859f7b8676`.
Browser already uses an iframe and the separate `t3-browser-page` hatch.
The clean standard bundle's actual module table has every requested factory,
and the obsolete roster warning is gone. Browser sources and the runtime pin
remain exact. All six root checks and 6,909 desktop TypeScript tests pass.
An isolated normal launch on port 16438 reports protocol 2 and the required
command-context capability, then quits normally with all recorded children.
The original server listener and clone window defaults remain unchanged.
Evidence is under `target/t3-poc-evidence/desktop-browser-roster-fix/`.
Independent review finds no introduced issue and passes nine existing Browser
tests. It verifies all 156 bundle files, the actual ABI table and baked identity.
Browser and normal-app chat UI acceptance remain open.

The existing Exact Mac controller now passes isolated project creation and draft
recovery. All 156 files and the source receipt match the clean `859f7b8676` bundle;
only this handoff differs at HEAD. Onboarding skips history import. The actual
palette creates one scratch project on its own authenticated server, and the
composer retains an exact unsent draft after normal Quit and an explicit project
reopen with a different seed. App state, tree and inspected native view captures
agree. This proves named agent scratch persistence. The controller's source sets
`context.agent=true`. Normal-app storage and Keychain persistence remain separate.

Through that controller, authorized Codex with the advertised GPT-6.1-Sol model
creates a real thread and returns the requested unique reply. A later assistant
response grows from 1,376 to 3,732 characters while running. The actual Stop control
produces Run interrupted. Its 5,427-character result stays unchanged over four
seconds. After another normal Quit, explicitly reopening the created thread
restores the reply, interrupted response and exact unsent composer draft in the
same named agent store. All owned processes exit. The original port-3773 listener,
clone window defaults, all product sources and all 156 bundle files remain exact.
Native OS chrome, physical keys, IME and normal-app credential restart behavior
remain unqualified. Evidence and limits are under
`target/t3-poc-evidence/desktop-exact-controller-acceptance/`. Desktop parity remains
incomplete. Mobile driver work and TestFlight remain unchanged. Independent
review finds no introduced issue, verifies the genuine reply, streaming, active
Stop and exact recovery, and passes 373 read-only artifact comparisons. Its
comment audit is clear.

Desktop header mouse input now reserves existing press regions from AppKit's
window movement handling. The computed NodeView override returns false for
buttons, existing press owners and their physical descendants. Blank views retain
the superclass answer. Pointer-only root listeners and semantic roles gain no
reservation. Disabled and inert controls keep their structural reservation while
existing input guards still refuse activation. No classifier, focus rule, chrome
or agent input path changes.

The transparent full-size titlebar regression misses its second application
click on unchanged production and passes with the fix. All 96 selected native
tests pass, covering first mouse, focus, field editing, caret and text selection.
Live handler removal, physical reparenting and a lifted popover's logical parent
also pass. Blank-region property comparisons qualify policy, not physical dragging.
All six root checks, 6,921 desktop tests and strict application TypeScript pass.

The clean standard bundle from `2ea073b284` passes the original header contact at
`[1253,26]` through the supported Exact Mac controller. The visible chooser opens,
its actual panel control closes it, and the remounted header control reopens it.
The original thread-details header control opens its popover; an outside contact
dismisses it. Body Add Project opens its palette. Every measured contact has one
journaled press, a complete zero-drop interval and matching state and inspected
native captures. No provider conversation is created and the composer stays empty.

All 156 bundle files, 3,513 actual inputs, ABI values and the strict signature
verify. Only MouseEventsMac.swift differs in the baseline and final input
manifests. No temporary production probes were added. Normal Quit stops the app
and backend before controller close. Owned processes exit; the original listener
and window defaults remain exact. Evidence is under
`target/t3-poc-evidence/desktop-header-drag-fix/`. Independent review of
`cee72da49c` finds no introduced P1 or P2 issue and reruns all 96 native tests.
It verifies the actual signed bytes, complete journals, seven captures and
cleanup. Its comment and suppression audits are clear. The earlier natural-query
experiment remains under `desktop-header-drag-policy/`.

This qualifies named agent Mac input with `context.agent=true`. Ordinary mouse,
physical window dragging, normal-app Keychain restart and full desktop parity
remain unqualified. Other titlebar input, including standalone selection, native
editors, value controls, raw canvases and reorder grips, remains unqualified.
Mobile and TestFlight are unchanged.

Draft Browser file assets now retain captured workspace provenance separately from
Browser ownership. The clean standard bundle from `56f150341d` passes the original
native Files flow through `Alt+Meta+B`, the actual HTML tree row and the existing
preview-browser control. The projection-error toast is gone. A real session,
frame, partition and `browser-page` guest show the owned file content while the
persisted thread ID stays empty and the exact unsent draft remains intact.
The same reserved Browser UUID survives navigation to owned A, its real link to B,
Back, Forward and settled Reload. Actual HTTP requests and a new response and
document token prove Reload. Inspected WebKit-composited captures agree.

Post-Browser file-row input now works in the named agent Mac flow. One native
probe identifies the inactive Browser's inner wrapper as the HTML click receiver.
The iframe is hidden, but its two boxes remain displayed with automatic pointer
input. The row's press is absent. Moving the existing `shown or behind` display
condition to the outer box hides ordinary inactive wrappers and preserves the
held-frame layout policy. Both temporary Swift probes are restored byte for byte.

The clean standard bundle from `19f33f164c` passes the same click at the same point.
It journals the HTML row press and opens the file. Space on the real Browser title
visibly returns with the exact memo, document, frame, partition and history intact.
Browser-title Enter was an unsupported activation choice, not lost input. The
owned Markdown row also opens by mouse. Its render toggle and actual file-link
context menu open the HTML in a second Browser tab. Returning to the first tab
retains the same memo and document. Both flows keep the exact unsent draft and an
empty persisted thread ID. No provider conversation is used.

All 540 Browser tests, 6,921 desktop tests, strict application TypeScript and all
six root checks pass. Independent wrapper review finds no introduced defect,
repeats all 540 Browser tests, and verifies the actual receiver, visible retention,
signed bundle, probe restoration and cleanup. Its comment and suppression audits
are clear. The earlier asset correction's independent review found no introduced
defect, passed 123 existing tests and 13 boundary probes, and verified its signed
bundle and native records. Its comment audit was clear.

The current receiver proof, red and green journals, captures and clean artifact
identity are under `target/t3-poc-evidence/desktop-browser-wrapper-fix/`. All 156
bundle files and 3,513 actual inputs verify. The prior `56f150341d` bundle is
preserved byte for byte. Earlier asset evidence remains under
`desktop-draft-preview-implementation/`, and its historical failure remains in
`desktop-native-browser-shortcut-acceptance/`. Owned processes exit, the original
listener and window defaults remain unchanged, and the isolated ports are free.
Ordinary mouse input, primary-launcher parity, normal-app storage and cookie
restart, physical keys and full desktop parity remain unqualified. The held
recording, separate-window and mini-frame paths are not requalified natively by
this unit. Mobile and TestFlight are unchanged.

The earlier standard desktop bundle from clean source `667c11267d` uses C ABI 18,
runtime 5 and Apple native ABI 3. Its executable SHA-256 starts
`ace5a0a9533f05a3`; its module starts `1dda02be376510c2`. The official pinned
runtime stages with matching archive, committed and release checksums, all 11
Mach-O signatures, version and scratch startup. The development build retains
its signing, package qualification and production-home guards. That artifact warned that
`t3-browser` is declared but absent from the native module; Browser behavior
remains unqualified.

Normal startup refuses a missing lane port. It also correctly refuses a server
folder physically under this checkout, because the checkout is inside `~/.t3`.
The owned QA directory stays under `target/scratch`; its server link resolves to
a fresh OS temporary directory outside that protected home. On two normal
launches with identical isolation, the app installs its own server, authenticates
automatically and reaches ready on loopback port 16437. Protocol 2 and
`serverResolvedCommandContext` are verified. Normal Quit stops the app, server
and recorded children. The second launch retains the same environment ID.
This proves bootstrap restart, not conversation or draft persistence.

The earlier normal-app UI drive stops at Connect, step 1. Agent-device reads that normal
AppKit screen, but screenshot capture times out. Foregrounding and the advised
wait both leave `RUNNER_BUSY`; the driver approach stops after three attempts.
No screenshot succeeds. Onboarding choices, project creation, advertised model
selection, provider readiness, Send, changing assistant text, Stop and transcript
or draft persistence remain unqualified. No project or thread is created. Clone
window defaults are restored, the original server listener is unchanged, and all
owned processes are closed. Evidence and exact commands are under
`target/t3-poc-evidence/desktop-native-baseline-acceptance/receipt.md`.
That startup baseline changed no product source. Mobile UI work stays blocked,
full platform parity remains incomplete, and TestFlight is unchanged.

Fresh normal iPhone builds use C ABI 18 and runtime compatibility 5, retaining
Apple native ABI 3 and the qualified 36-tag roster. EAS passes native Home controls,
Settings/Appearance/Back, Search typing and clearing, app keyboard dismissal,
New Task/Back and ordinary relaunch, both before and after the private fixture.
Return preference and base text size persist across restarts; starting values are
restored. At 16-point light appearance, the supplied-data workspace and attachment
previews retain exact code and displayed Source through legal-edge and return
checks, with 14-point end padding and zero reference-pixel mismatches. The translated
HDR shadow reaches a stable idle state. All temporary sources restore exactly;
the final normal build is clean and the EAS session is shut down.

Evidence and limits are in
`target/t3-poc-evidence/main-refresh-20261030/integration/receipt.md`.
Connected cloud routes, hardware input and full phone, iPad and desktop parity
remain unqualified. Independent integration review finds no introduced P1 or P2
issue. Its 25 focused tests, source and artifact comparisons, and raw device
checks pass. No comment cleanup is required. The historical real-provider POC
source is preserved; this refresh does not repeat connected acceptance. The
delivered TestFlight build is unchanged, and this integration reaches no new
release checkpoint.

Direct text-only emphasis now renders in both existing file-table preview owners.
The adapter retains positioned decoded text, outer-child IDs and whole-segment
fallback; nested formatting remains unsupported. The exact reference DM Sans
400 and 700 italic assets are registered in the existing family. Actual cached
CoreText runs on EAS resolve body/header Latin text to DMSans-Italic and
DMSans-BoldItalic at 16-point light and 20-point dark appearance. Existing file
prose italics and the transcript's 500-weight nearest-face selection also resolve
to real italics. Emoji fallback is recorded separately. Asset-backed CoreText
specimens and upright controls qualify the table glyphs.

Both owners retain Source, returned Copy inputs and complete fences. Native word
selection and a bounded code-to-strong handle drag are reachable; selected-copy
bytes and guarded Copy execution remain unqualified. Wide tables pass legal-edge
repeats and returns with zero horizontal range outside the table. Unsupported
nesting retains its whole prose segment, and the unpaired control remains Source.
The pinned fonts lack tabular digits despite preserved numeric style requests;
tabular rendering remains unqualified. All 4,170 mobile tests, 276 focused tests,
strict TypeScript, generator verification and all six root checks pass.

The fresh normal build from clean functional commit `da653334f7` passes Home,
Settings/Appearance/Back, Search typing/clearing and app keyboard dismissal,
New Task/Back and ordinary relaunch with a changed PID on EAS. Preferences are
unchanged. Its native module hash and ABI 18/runtime 5/Apple ABI 3/36-tag roster
remain qualified. Every temporary entry and observer restores exactly; the EAS
session is shut down. Evidence and limits are in
`target/t3-poc-evidence/phone-emphasis-table-implementation/receipt.md`.
Independent review finds no introduced P1 or P2 issue. It passes 275 focused
tests and a separate comparison of all 182 generated outputs, replays complete
owner projections, and verifies raw native evidence, font files and bundles.
No comment cleanup is required. Connected cloud routes and complete phone,
iPad and desktop parity remain open. TestFlight is unchanged; this slice reaches
no release checkpoint.

Selected-text Copy acceptance remains open. A fresh EAS baseline reuses the clean
`da653334f7` normal artifact after a complete source and bundle hash comparison.
Home, Settings, Appearance and Back work. The private observer records an actual
`bold` selection at UTF-16 range `[21, 4]`, but no native Copy tap completes.
Its timer remains armed after that aborted case and attempts to read the next
known sentinel, opening iOS's paste-permission dialog. The driver times out and its one alert
recovery fails. Device work stops and the scoped EAS session shuts down.
All temporary production sources restore exactly. The corrected private observer
compiles but has no device qualification. Post-fixture normal smoke, selected-copy
bytes for both owners and guarded Copy execution remain unqualified. No production
Copy defect is established. Evidence is in
`target/t3-poc-evidence/phone-selected-copy-acceptance/receipt.md`.
Production behavior and TestFlight are unchanged.

Startup-crash parity remains open as GAP 004. The pinned reference reads fatal
startup records from expo-updates; Exact exposes no equivalent record producer.
The unavailable state remains truthful. Source findings and the required capture
boundary are in `target/t3-poc-evidence/phone-startup-diagnostics-grounding/how.md`.

The native integration passes emoji editing, Photos import and cancel, inline
preview and Back with unflushed text preserved, explicit image removal, native
text Undo and Redo, model and branch round trips, plain-text paste, Unsent
recovery, ordinary composer regression and real Stop. The seven integration
commits end at `7144c25dd4`. Independent review passes 678 focused tests and
reproduces three defects. Fixes in `425d03604e`, `b3c2aea32f` and `040848296d`
resume editing after a stale preview refusal, preserve loaded Plan-mode and
import policy, and retain producer errors until Root claims them. Each regression
fails before its correction. All 4,088 mobile tests, strict TypeScript, boot and
the normal iPhone build pass. The build reads the native module roster and
preserves package qualification. The four redundant comments identified in
review are removed.

Picker focus still needs an editor tap after three bounded attempts. Files
completion remains unproved after three attempts that include selection-epoch
changes. Photos import works with delayed completion. Picker-replacement Undo,
hardware and alternate submit, live IME, environment or project changes during
awaited work, and actual voice remain unqualified. Image-only paste directs the
user to Photo Library. These limits prevent a phone parity checkpoint.

The isolated native editor fixture now passes 142 checks on EAS, with zero
failures, against a 124-check baseline on the same cloud device. Eighteen new
checks dispatch the registered Return actions through UIKit. Both Return
preferences deliver ordinary and alternate submit with the latest unacknowledged
source and emoji caret, refuse cached actions during composition, freeze and
deactivation, and resume submission after source acknowledgment. Build inputs
and executable hashes are recorded. This verifies native responder behavior;
actual hardware input, Root/outbox dispatch and connected app acceptance remain
separate. The cloud session is shut down. Production Swift is unchanged.

T3's EAS cloud host installs and opens the normal bundle and presents the initial
environment form. Host entry works, an invalid address shows an inline error,
Back returns Home, and relaunch restores the empty Home. It starts without the
locally saved pairing. Connected cloud
flows and a cloud-reachable authorized server route remain unverified. The cloud
session was shut down after this check. The local simulator's existing catalog
and credentials remain intact. This integration is outside the delivered
TestFlight build.

Home chrome and keyboard commands now wait for nonempty resource configuration
before mounting their native views. Home also requires its mounted route key.
The captured EAS cold-start failure passed empty placeholders to both factories;
later valid props could not recover those refused nodes. A clean-install EAS
readout now shows both factories ready after resource completion, with the same
native module hash. The normal build independently passes Home title and controls,
Settings and Appearance with Back, Search focus, typing and clearing, New task
and Back, and ordinary relaunch. The component regressions fail the former mounts.
A normal rebuild from clean commit `4e27642c87` repeats that entire flow after
uninstall and reinstall. The EAS session is shut down.
All 4,091 mobile tests, strict app and owner TypeScript, and the six repository
checks pass. Host diagnostics are restored byte for byte. Connected cloud flows,
hardware keyboard input and full phone parity remain unqualified. This fix is
outside the delivered TestFlight build.
Independent review finds no introduced P1 or P2 issue and repeats all 83 focused
tests. Its five stale comment lines are removed; both decision references remain.
The rebuilt normal app preserves Dark appearance across a process restart.
Restoring System appearance persists across a second restart. Both restarted
processes retain the native Home controls on EAS.

The normal EAS build from `af1afbe000` also preserves base text size 22,
custom code size 18, custom terminal size 14 and Word break after a process
restart. Preview wrapping and Home navigation remain usable. Restoring base 16,
automatic code and terminal sizing, and Word break off persists through another
restart. Defaults are restored and the cloud session is shut down. Evidence is
under `target/t3-poc-evidence/phone-typography-preferences/`. This qualifies
offline preference behavior, not quantitative font or connected terminal parity.

The normal build from `c102ae291c` also passes Return-key preference persistence
on fresh EAS. The observed starting selection is Send message. Insert new line
remains selected after leaving and reopening Keyboard and after a process restart.
Restoring Send message persists after another restart. Accessibility selection
and inspected native checkmarks agree. Home controls, Settings navigation,
New task and Back remain usable. Evidence is under
`target/t3-poc-evidence/phone-keyboard-preference-persistence/`.
This qualifies offline preference storage and the choice UI. Hardware input,
IME, Return dispatch and connected submission remain separate. The cloud session
is shut down. No product code changed, and TestFlight is unchanged.

The same normal build passes About and license navigation on fresh EAS. About's
Version `0.1.0` matches the bundle. Searching `shikijs` shows `@shikijs/langs`
and `@shikijs/themes`, both `4.2.0 · MIT`. Opening `@shikijs/langs` shows Mobile metadata.
Its complete notice matches bundled text after observed accessibility line-break
normalization, and the rendered ending is visible. Back retains the query and both
results without re-entry. An unmatched query shows no-results feedback. The app's
Clear text control restores the list. About, Settings, Home search clearing,
New task and Back remain usable. Evidence is under
`target/t3-poc-evidence/phone-information-navigation/`.
This qualifies local information navigation, not complete license coverage, range
selection, connected behavior or broader parity. No product or test code changed.
The cloud session is shut down, and TestFlight is unchanged.

Text-only strong table cells now render through the existing native text runs.
The generated adapter admits direct strong nodes with positioned plain-text
children and retains whole-segment fallback for unsupported content. Existing
plain/code records, Source bytes and fenced-code identities remain exact.
The matched standard-host fixture mounts the actual workspace and attachment
components on EAS. Both render decoded mixed cells and strong headers at
16-point light and 20-point dark appearance. Native word selection is reachable;
cross-run selection and guarded code Copy execution remain unqualified. Wide
tables pan to their edges while surrounding prose stays fixed. The unpaired
attachment control still selects Source. All 4,118 mobile tests, 218 focused
tests, strict app and owner TypeScript, generator verification, desktop minimap
tests and six root checks pass. The separately rebuilt normal app passes Home,
Settings, Search, New task and relaunch on EAS. Both entry files restore exactly,
and the normal native module retains its prior hash. The private module differs
as in the accepted baseline; its cause remains unestablished. Evidence is under
`target/t3-poc-evidence/phone-rich-table-final/`. Connected file reads, auth, cache
lifecycle, persistence and full phone parity remain unqualified by this fixture.
This change is outside the delivered TestFlight build.

Independent strong-table review finds no introduced P1 or P2 issue. It repeats
218 focused tests, 40 source probes and eight production snapshot comparisons,
and verifies artifact and restoration hashes. The input-only assertion and two
redundant comments identified by review are removed. Source and Copy checks
continue to assert returned production values.

Formatted file paragraphs now join parser-certified soft breaks while hard breaks,
paragraph gaps, Source/copy bytes and complete code objects remain exact. The
adapter migration first matches all 2,196 captured production results. All 4,153
mobile tests, 253 focused tests including desktop minimap, strict TypeScript,
generator verification and the six repository checks pass. Both actual preview
components pass baseline/current EAS controls at 16-point light and 20-point dark.
Heading/table controls, natural wrapping and single code pans remain usable.

Baseline native measurements proved the long-code blanking mechanism. The raster
was marked whole and settled at 592 points despite uncut paint widths of 787.54
and 969.28 points. Legal inner scrolling exposed those missing pixels. Later
outward pans chained into the fixture root. Those measurements did not qualify the
separate end-padding discrepancy. Source and the temporary observer are restored exactly.
Evidence is under `target/t3-poc-evidence/phone-code-native-metrics/`.

The painting correction now retains measured coverage in each raster result.
Both real preview owners pass initial, middle, legal edge and return pans on EAS,
with exact source identity, one current image and zero CoreText pixel mismatches.
Private native huge-line and tall-wide jumps also answer current demand with
finite windows and idle convergence; paused-worker reversal keeps admitted work.
Shared Apple tests pass; UIKit tests compile, and the shared tvOS target builds.
Evidence is under `target/t3-poc-evidence/phone-code-raster-implementation/`.
Independent review verifies artifact hashes and identifies a distant HDR-shadow
dependency that could disappear from partial windows. The correction captures
those casters in translated source coordinates with physical-pixel alignment.
Sixteen native shadow cases pass direct pixel comparisons at 1x and 3x; EAS also
passes the fractional-offset case with zero mismatches and idle convergence.
Oversized dependencies decline partial admission instead of claiming coverage.
All 43 selected shared Apple tests, 4,153 mobile tests, strict TypeScript,
generator verification and six root checks pass. UIKit owner tests compile with
restored allowance and urgency assertions; they are not executed locally. The
shared tvOS target builds. Thirty-two stale or redundant comment lines are removed.
The clean normal build from `7060041026` passes Home, Settings, Search typing and
clearing, New task, Back and process restart on EAS. Both preview edges repeat
with complete final words and zero pixel mismatches. Evidence for this review
correction is under the raster implementation's `parent-review-fix/` directory.
End padding is corrected below. Fixture-root chaining remains open.
Entries, normal declarations and temporary observers are restored; the cloud
device is shut down.

The shared code scroller now uses a flex row and retains 14-point trailing padding
after its allocated paragraph. The workspace preview at 16-point light and the
attachment preview at 20-point dark pass initial, middle, legal-edge and return
pans on EAS. Source and forced lines remain exact, with one image and zero pixel
mismatches per flow.
Short code keeps its card and scrollport dimensions and has no horizontal range.
Native word selection remains reachable; guarded Copy execution remains unqualified.
The normal build from `c102ae291c` passes Home, Settings, Search typing and clearing,
app-owned keyboard dismissal, New task, Back and process restart on fresh EAS.
All 4,153 mobile tests, strict TypeScript, generator verification and six root
checks pass. Evidence is under
`target/t3-poc-evidence/phone-code-end-padding-implementation/`.
The fixture toolbar still permits 37.6667-point root travel after excess pans.
Final edge captures restore only that root through a fresh accessibility header gesture.
Connected cloud routes and full phone, iPad and desktop parity remain unqualified.
This slice reaches no release checkpoint. The TestFlight build is unchanged.
Independent review finds no introduced P1 or P2 issue. It verifies both signed
bundles, the exact one-property source change, native metric replay and accepted
owner captures. No comment cleanup is needed.

Formatted-prose evidence is under
`target/t3-poc-evidence/phone-formatted-prose-implementation/`. Supplied-data
rendering does not qualify connected reads, auth, cache, persistence, quantitative
font parity or full selection parity. Independent review passes 253 focused tests,
80 boundary probes and all 2,196 saved production results. Its heading-heavy CPU
finding is corrected by fragment-local break lists and one ordered range walk.
All captured results remain exact, and all mobile tests and repository checks
pass again. The normal build from `f349a4d50a` repeats Settings, Search, New task,
Back and restart on fresh EAS. Native large-document latency remains unqualified.
This slice reaches no release checkpoint and is outside the POC TestFlight build.

These earlier implementation checkpoints are available on GitHub:

| Work | Commit | Branch |
| --- | --- | --- |
| Root app modules and compiler fixes | `61b7997` | [Root checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-root-checkpoint-20261028) |
| App refactors and focused tests | `0e6c9ae` | [App checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-contract-apfs-r1-20261024) |
| Mobile implementation and release work | `250d863` | [Mobile checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-draft-apfs-r1-20261024) |

[PR216](https://github.com/ccheever/exact2/pull/216) remains a draft against
`daehyeon/t3-code-main`. Keep it unmerged and limit distribution to TestFlight.

- The desktop Root now uses five feature-local Contract `members` files and is
  1,382 lines. All five extraction steps compile to the exact original plan.
  Root retains state, request ownership and declaration order. The fixture
  assembles members at their listing positions. All 7,159 desktop TypeScript
  tests, 19 compiler members tests, strict application TypeScript and the six
  repository checks pass, including caps. With `BUN` explicitly set to the
  pinned executable, the complete desktop `tests/` command passes 6,909 tests,
  including the previously unregistered `.mjs` adapter test. The normal native desktop
  bake initially times out after 1,200 seconds. A build with
  `CARGO_PROFILE_HOST_DEV_BUILD_OVERRIDE_OPT_LEVEL=3` succeeds and preserves
  the clone identity and native package qualification. The signed bundle
  launches through the repository driver in a fresh scratch store. Its
  onboarding help and invalid-link error work. All five members are registered
  as Cargo inputs, and editing each makes the native driver refuse the stale
  build. Source bytes are restored exactly. The resulting rebuild succeeds;
  the rebuilt bundle has no stale inputs and launches again. Connected desktop
  UI remains unqualified. The five
  extraction commits end at `823073b67c`. Independent
  review finds no introduced P1 or P2 issue. It passes a fresh full-plan
  comparison, 560 desktop tests, 19 compiler members tests, caps and 11 fixture
  reader probes. No new comment cleanup is required.
- Focused compiler, chat and command tests passed. Whole-app behavior, visual
  parity and final integrated checks remain unverified.
- The InlineShortcut comparison has no complete verdict. Its cached test
  executable is missing, so another test run needs a fresh build.
- Live App Store Connect status on October 28 reports POC build
  `1.0.0 (20261029015732)` as `VALID` and `IN_BETA_TESTING` internally. Its exact
  build is assigned to `Team (Expo)`, with one installed tester and one invited
  tester. External distribution was not extended.
- The observed ordinary Send stall hit the 100 ms JavaScript turn budget.
  Duplicate configuration scans are removed while full change, authorization
  and receipt checks remain. Eight consecutive fresh sends, growing output,
  Stop and restart pass in the normal iPhone app. Bounded cache transfers also
  address the separate 4 MiB native-request refusals.

An earlier integration incorporated remote `main` at `1c7b918556`. Five conflict
resolutions preserve both the clone's production package qualification and
main's development flags, system-framework linking and source-sweep exclusions.
The clone lockfile adds the three dependencies its updated path crates require.
All 3,871 mobile tests, strict TypeScript, boot, Rust build/tests/Clippy/formatting,
workspace compilation and the quick semantics comparison pass. The builder suite
has 99 passing cases and one skip after two timeouts pass in a focused rerun.
Caps then reported only the inherited desktop Contract violation. Independent
review finds no integration defect. The normal merged iPhone build restores the
saved environment and conversation, receives `MAIN_SYNC_OK`, displays growing
output from 416 to 818 characters, and stops with server-confirmed interruption.
After restart, the interrupted conversation reopens with an empty composer and
receives `MAIN_RESTART_OK`. This integration is outside the delivered TestFlight
build below. Full phone parity remains open.

A later main refresh merged without conflicts. Its symbol fixtures now register
handwritten images with the runtime and supply its current browser hooks. All 31
affected symbol, paint and list tests pass, including an independent rerun. Rust
build/tests/Clippy/formatting, workspace and boot pass. Caps then retained the desktop
violation, now resolved above. The broader web run has 136 passes and two failures. Its cold-build
timeout passes on rerun; the remaining motion check cannot link because the
installed nightly's `rust-lld` cannot locate `libLLVM.dylib`. Three attempts stop
at that toolchain error. Firefox is unavailable. The rebuilt normal iPhone app
restores the saved environment and conversation and receives `MAIN_REFRESH_OK`
from the real provider. This refresh is outside the delivered TestFlight build.

Application tests, technical drafts and local build evidence are retained.
Consult them for specific implementation questions rather than as a task queue.

The ordinary iPhone simulator build pairs with the real local T3 environment.
Removing unused per-row Arrange
planner probes fixes Home navigation on the live catalogue. The app opens a
project, creates a thread, sends a message, and displays the real provider reply.
The environment and that conversation remain available after an ordinary restart.

A separate root resource now loads complete Arrange data only while Arrange is
open. Ordinary Home replies no longer include its rows or catalogue version.
The normal iPhone build sends a follow-up, displays growing streamed output, and
stops it; the server confirms an interrupted run. The earlier Hermes abort has
not recurred in this drive. Text observations now commit independently of the
bounded command queue; lifecycle, submit and command outcomes remain ordered.
The normal integrated PR build saves all 124 characters of a rapid edit exactly
and restores that complete draft, saved environment and conversation after relaunch.
The restored draft reaches the server intact. Final acceptance on an available
existing checkout creates a new thread, receives a real reply, sends a follow-up,
and stops its provider run. After relaunch, the environment, reply and interrupted
conversation remain available. The functional iPhone POC is verified; its signed
production build is signed and available to the existing internal TestFlight group.
The first Home fix is pushed in PR216 at `122646f95` and independently reviewed.

POC build `1.0.0 (20261028074348)` comes from pushed commit `d030e5536`.
The IPA passes signature and provisioning checks with the existing clone identity.
Its production build reads the native module table on the iOS Simulator after
compiling package sources for that probe's target. The builder regression suite
passes 49 tests with one skipped. The earlier device-only package output blocked
all roster probes; the fix preserves production qualification.
[EAS submission](https://expo.dev/accounts/exponathan/projects/t3-code/submissions/7e93de65-ae93-431c-b5ce-0be7874b3fc7)
uses that exact IPA and the existing internal `Team (Expo)` group. Apple processing
is `VALID`, internal state is `IN_BETA_TESTING`, and the group includes this build.
Its App Store Connect build ID is `4161d4c0-078f-4952-9927-cf98b251ab23`.
The IPA SHA-256 is `f60ac86249f6744220b4f4751a25789decf62cc66ef0e72aab5f30ba4bec24d1`.
The POC TestFlight checkpoint is delivered. Full phone, iPad and desktop parity
remain incomplete and need their own acceptance evidence.

POC reliability build `1.0.0 (20261029015732)` contains clean pushed source
`38fd0f766a5352e48ae65d4b168854b5e754747f`, including the complete cache transfers
and Send continuation correction. Signature, store profile, identity, version
and source-stamp checks pass. The exact IPA SHA-256 is
`b1b31ae604d7c88e28cb47b28049b4a661f0d603cd33fdc07aac34b0223f173e`.
[EAS submission](https://expo.dev/accounts/exponathan/projects/t3-code/submissions/3992b0fd-7fad-454c-a89c-46156667d65a)
submits that IPA. Apple build `9a6d8e6f-2cfe-4097-9580-f96f50fb2970` is `VALID`
and `IN_BETA_TESTING`. The existing internal `Team (Expo)` group includes this
exact build, with one installed tester and one invited tester. Installation of
this new build is not verified. External distribution was not extended.
The normal app creates a fresh thread in the existing `main` checkout and receives
`POC_RELIABILITY_OK`. A production simulator build from the same commit restores
that environment and thread, receives `PROD_POC_OK`, displays growing output from
139 to 1,126 characters, and stops with server-confirmed interruption. After
restart, the interrupted conversation and empty composer return. Its simulator
package has a separate version stamp; the uploaded device archive is verified
independently. This refreshes the delivered POC; full phone parity remains open.

The phone parity drive found and corrected a direct-pairing difference: the pinned
native reference accepts the Mac's LAN HTTP address while the clone rejected it.
Mobile now accepts that origin and leaves iOS App Transport Security in force.
The normal clone pairs over LAN, saves that address, restarts, restores the
conversation, and receives a fresh real provider reply. Independent source review
found no introduced defect. The TestFlight build above includes this correction.
Physical-phone pairing remains unproved.
An unreachable-address drive now shows the saved environment's reconnection
status without a loading spinner. Its header opens Environments. Native retries
retain the failure reason; presentation resolves the saved owner before server
discovery while the live client remains unidentified. Restoring the LAN address
and restarting clears the failure header and restores the existing conversation.
Source tests cover pending ownership, foreign/ambiguous origins, background
failure and recovery. The shared source pins are unchanged.
This drive changes the saved origin, which invalidates its old local cache; it
does not qualify cached conversations during a real network outage. The simulator
Local Network settings page has an empty app list, so it provides no clone-specific
toggle for that acceptance. No permission or saved origin changed. Canceling
the system file picker restores an enabled composer.

The normal iPhone build imports a 2 KB Markdown file through Choose Files, opens
its draft preview, and retains the file after restart. Sending uploads the file to
the real environment and receives `ATTACHMENT_POC_OK`. After another restart,
the sent attachment opens from the saved conversation. Inline file-chip taps now
open that same draft preview and Back retains the attachment. Opening first saves
observed text, then rechecks the editor and file; a blocked-save regression covers
rapid typing and route/file changes before navigation. Stale mounts, token ranges,
draft text and removed files cannot open it. Other rich context actions remain incomplete. Sent Markdown attachments now use the
workspace file code cards: Preview/Source round trips, horizontal code scrolling,
and the exact 344-character code-body copy pass on iPhone. Preview parsing accepts
CRLF/CR while Source retains original newlines. Generic Markdown prose,
complex tables and images still differ from the reference. These later changes are
outside the POC TestFlight build.

The ordinary thread also imports a 567,032-byte PNG through Files. Its restored
draft opens in Quick Look; sending receives `IMAGE_ATTACHMENT_OK`, and the sent
image opens after restart. Preview resolution now follows image context to its
file-backed bytes; removing the reference or file prevents a new preview.
Ordinary-chat Photo Library selection is now verified in the normal build. The
system picker imports the test PNG as `IMG_0007`; its draft and strip preview
survive restart. Sending it with an explicit message receives `PHOTO_LIBRARY_OK`
from the real provider. After another restart, the saved conversation opens the
sent image correctly. Inline image chips now open the bound draft preview after
saving observed text and rechecking the current context and attachment inventory.
The normal build using committed Swift sources opens the photo from its chip;
closing the preview preserves newly typed text and the attachment. Removing the
photo preserves that text. Source tests cover snapshot and file-backed images,
including attachment or context removal during an awaited save.
New-task drafts import and preview the same PNG, recover through Home's Unsent
row after restart, and discard through confirmation. Unsent follows active tasks
once the live catalogue loads. Current checkout selects `main` correctly. The
new-task editor still displays raw attachment references. A later drive types the
message before importing the PNG, sends on the current checkout, and receives
`NEW_TASK_IMAGE_OK` from the real provider. After relaunch, the saved environment,
conversation, reply and sent PNG preview return. The server records the new thread
as mobile-created on `main`. Earlier automation text replacement failed, which
alone does not establish an app typing-loss defect.
New-task dictation now captures the saved text/revision before readiness and reads
that monotonic revision at completion. Regression tests cover edit/revert during
readiness and between voice observations, plus an unchanged draft. The two stale
cases fail against the previous code. Native dictation itself remains unqualified.
NewTask Photo Library import also survives restart and reaches the real provider
with `NEW_TASK_PHOTO_OK`. The original admitted message recovers on relaunch with
one mobile-created thread and one completed run. A later normal build restores
the environment, conversation, response and sent image. Delivery initially hit
the JavaScript turn budget. An instrumented drive finds repeated connection
serialization of about 3.77 million characters. Outbox rendering now computes
that stamp only for waiting tasks that need it; those tasks retain the complete
comparison. Its regression fails the previous source, and all 40 outbox tests pass.
Strict TypeScript, boot and the normal native build pass.
The cache preserves complete format-2 catalogs and their exact encoded text.
Private transfers now bound large writes, reads and conditional removal without
changing caller APIs, SQLite, tickets, CAS or the store's 4/16 MiB payload limits.
Transfers are atomic, scoped to their owner, bounded in memory and expired while
idle. Clears retire matching transfers; admitted forget cleanup still finishes
after module teardown. The real TypeScript-to-Foundation path round-trips exact
4 MiB shell and 16 MiB thread values, preserves duplicate-write timestamps and
replacement-safe deletion, and returns all 128 metadata rows above 1 MiB. The
native transport fixture passes 9,559 checks; the unchanged store passes 86.
All 3,871 mobile tests, strict TypeScript, boot and the normal native build pass.
Independent review finds no introduced blocker. The normal iPhone app freshly
stores its complete 4,096,034-byte configuration. Its captured trace has no 4 MiB
request refusal. A separate immediate capture identifies the Send continuation
refusal described below. Actual delivery still compares complete configurations.
No phone parity or new TestFlight checkpoint is claimed.
The catalog reader distinguishes unread from successfully loaded empty state and
retains readiness through later failures. A native launch trace found restored
reader readiness from the previous session reaching a cold JavaScript client.
Home therefore briefly reported No environments connected. Readiness now travels
with the current rendering snapshot; the reader exposes only its invalidation
serial. The regression evaluates the production Home expression with a restored
reader and fails the former code. Failed, malformed, superseded and successful
empty reads remain covered. Two normal retained-cache restarts show Loading
environments followed by saved threads, and the interrupted conversation reopens.
All 3,727 mobile tests, strict app TypeScript, boot and the normal build pass.
Source review finds no introduced blocker. Temporary probes are removed. This
correction is outside maintenance build `20261028202307`.
Ordinary Send now captures only its policy fields before validation and copying.
Workspace skills, commands and snapshots do not enter that capture. It retains
upload limits, selected model availability and toggle settings, driver fallback,
and complete pooled provider usage data. The three reads and synchronous ownership
boundary remain unchanged. This corrects the observed 102.4 ms pre-admission turn.
The preserved 64-character draft reaches the real environment once and receives
`CACHE_ROUNDTRIP_OK`. The normal build displays the reply and restores it after
restart. All 3,727 mobile tests, strict TypeScript, boot and the native build pass.
Independent source review finds no introduced correctness blocker. Its redundant
comment is removed. Delivery still performs its existing full-config comparisons.
A fresh normal-build drive at `7c1a11570` opens the real project, creates a
mobile thread on `main`, and receives `POC_REFRESH_OK`. Follow-ups reach the
provider. Stop displays Run interrupted, and the server confirms interruption
with no active run. After ordinary restart, the saved environment, conversation,
and interrupted state return. A later clean normal build at `d3fb4f59d` displays
growing paragraph output, then stops it with server-confirmed interruption.
The interrupted reply survives restart. A follow-up receives `REPEAT49_OK`.
The next follow-up reproduces the Send stall; restart delivers that original
message once and receives `REPEAT50_OK`. Its draft remains empty.
Continuous plain lines did not supply an intermediate Markdown block for the
earlier streaming checks. The pinned server filters streaming output by complete
blocks in paragraph mode. No environment setting changed during these drives.
All temporary probes are removed. The clean build contains none of their
diagnostic strings across its eight Mach-O artifacts.
An immediate normal-app trace catches an outbox continuation taking 103.7 ms
against Exact's 100 ms budget. Its saved native reservation survives the refused
callback. Outbox validation now reuses checks within each synchronous segment
and retains complete config/scopes comparisons after awaits. The two work-count
regressions fail the former source. In-place changes to unknown config fields
and scopes still block issue. Independent review finds no introduced defect.
The normal build recovers the original reserved message once, then receives
`SEND53_OK` through `SEND60_OK` in eight consecutive fresh sends without restart.
Four captured continuations have no refusal. The last three sends use no dev
menu. Growing paragraph output increases from 638 to 1,662 characters before
Stop; the server confirms interruption with no active or pending run. After
restart, the saved environment, interrupted reply and empty composer return.
The complete mobile suite passes 3,871 tests. Full phone parity remains open.
Signed archive `1.0.0 (20261028202307)` contains pushed source `7c1a11570` and
excludes the later native editor changes. Signature, store provisioning, identity, version and
source-stamp checks pass. The held files and original Git index restore exactly.
Its IPA SHA-256 is `f9b4ba61b9484d2c5163aa3940cab221195a5addf59340e07f20392dec11c85e`.
[EAS submission](https://expo.dev/accounts/exponathan/projects/t3-code/submissions/5acee506-6607-48d9-8e54-12b1f62fa043)
uses that exact IPA and the existing internal Team (Expo) group. Apple processing
is VALID and internal state is IN_BETA_TESTING. The group lists this exact build,
with one installed tester and one invited tester; installation of this new build
by an individual tester is not established. Its App Store Connect build ID is
`d1ba4f4f-ec9a-449d-9e26-f8a13c3e66f1`. Distribution remains internal TestFlight.
This POC maintenance build is delivered. Phone and iPad parity remain incomplete.
The inactive native NewTask editor reserves eight final-source slots and retains
teardown receipts until explicit acknowledgment. Its handoff freezes the writer,
retains the captured source until save acknowledgment, and resumes with the
producer's explicit document. Reservations follow stable draft identity across
transport generations. Rebased token arrays decide unchanged resumes and supply
Undo capture after native typing. The actual UIKit fixture passes 114 checks,
including chip restoration, producer Undo and strict unchanged-resume checks.
An isolated focused UITextView proves that the fixture's explicit empty Undo
group itself makes `canUndo` true. Removing that group preserves both strict assertions
and resolves the former 113-pass/one-failure result without another production
comparison change. The Foundation protocol and operation suites pass 37 and 84
checks. Independent source review finds no introduced blocker. The normal app
using these native sources restores an ordinary draft exactly after restart.
Clearing survives another restart, and the saved environment and conversation
remain available. No QA draft remains. The client-lifetime Root refresh now watches
and drains final editor sources after draft recovery hydrates. Its bounded owner
captures each draft incarnation and native mount, publishes retired source once,
saves through the existing preferences store, then acknowledges the exact native
receipt. Detached routes and older transport generations retain their captured
destination. Failed saves and uncertain acknowledgments retry with fresh IO;
frozen operations and unknown sources stay retained. The 30 focused tests cover
these transitions, and all 3,853 mobile tests, strict app TypeScript and boot pass.
The normal iPhone build restores an exact ordinary draft including an emoji;
clearing survives another restart. A NewTask draft restores through Home's Unsent
row, and confirmed discard survives restart. The saved environment and conversation
remain available. No QA draft remains. Independent source review finds no
correctness blocker. Caps then reported only the 2,994-line desktop Contract.
NewTask retains its
textarea. The exported native presentation remains unwired. Activation still
requires producer integration, token projection, voice and paste ownership
acceptance, and native node recreation after a factory refusal. Direct
editor-object retry alone does not qualify recovery through the host.
The existing native picker intake now admits a closed NewTask destination and
proves adoption against its exact saved record, source, inventory and mixed order.
It retains the existing operation, native claim and durable cleanup owner.
The UIKit fixture passes 149 checks, including frozen saved-file acquisition,
issued holds, refused ownership changes, retirement, failed saves, terminal
replay and cold cleanup for ordinary and NewTask drafts. All 3,871 mobile tests,
strict TypeScript, boot, the Rust checks and the normal native build pass.
Independent source review finds no introduced blocker. This verifies the native
boundary. App-owned TypeScript IO now accepts the same closed NewTask target
and full saved-record publication through its existing picker and hold lifecycle.
Ordinary history and runtime retain explicit thread-only admission. The focused
suites pass 134 tests; all 3,886 mobile tests, strict TypeScript, boot and the normal
native build pass. Removing the ordinary guard makes its new regression fail.
Independent review finds no defect and passes 21 additional boundary assertions.
The normal app imports the real 2 KB Markdown file, opens its preview, removes
it, and restores an empty composer after restart. The environment and conversation
remain available. Native NewTask producer integration and activation remain open;
its textarea is unchanged. This IO unit is outside the delivered TestFlight build.

The NewTask context owner now publishes captured attachment batches atomically.
It preserves the captured UTF-16 range, shelf-only image policy, mixed inventory
order, context history and latest draft settings. Independent review found an
unreferenced binding mismatch that could queue a live file for removal. Complete
saved binding validation now refuses that case before projection. The focused
suites pass 76 tests; all 3,903 mobile tests, strict TypeScript, boot and the normal
native build pass. Picker dispatch, durable finish proof and native editor
activation remain separate work. This reducer is outside the TestFlight build.

The native NewTask owner now reserves a captured Photos or Files operation,
freezes its source, persists it and acknowledges the exact receipt. Saved frozen
work and its disabled native node remain owned after acknowledgment. Root retries
uncertain work with fresh IO; confirmed slot release replaces an unacknowledged
factory seed once. Review found superseded request failures could disable another
editor. Regressions reproduce both allocation and freeze failures; guards now
ignore them after newer work has succeeded. All 3,927 mobile tests, strict
TypeScript, boot and the normal native build pass. Intake, holds, attachment
publication, finish and explicit resume are separate work below.
The textarea remains active. This checkpoint is outside the TestFlight build.
The normal build receives `FROZEN_CHECKPOINT_OK` from the real provider and
restores that reply and an empty composer after restart.

Native finalization reads now include every reserved independent editor identity.
Saved teardown removes its reservation and wakes the existing Root subscription
without republishing source. The NewTask owner consumes complete snapshots only
for publications captured before dispatch. It distinguishes a live saved slot
from release, retires the old presentation and retries a refused factory seed
once. The saved picker operation still blocks another writer for its draft.
The textarea remains active pending producer integration and resume acceptance.
All 3,939 mobile tests, strict TypeScript, boot, 88 native ledger checks and
117 real UIKit checks pass. The normal simulator build succeeds. Independent
review adds two passing ACK-overlap regressions and finds no introduced blocker.
The normal app receives `SLOT_RELEASE_OK.` from the real provider and restores
that reply and an empty composer after restart. This checkpoint is outside the
delivered TestFlight build.

Native picker status now distinguishes an absent exact identity, generation and
operation tuple from an ordinary failure. The observation writes no release queue
and performs no cancellation. Only status may return absence, with empty files
and error fields. The TypeScript boundary still requires the exact reply owner.
Existing ordinary finish and cancel paths require their own terminal outcomes.
The NewTask continuation will also need positive source-slot retirement before
using absence to resolve an unpublished operation. Published work remains held.
All 3,940 mobile tests, strict TypeScript and boot pass. The native coordinator
fixture passes 163 checks. Independent IO review passes 79 tests and finds no
introduced blocker. The normal build imports and previews the real Markdown
file, restores it after restart and opens its preview again. Removing the test
file restores an empty composer through another restart. The environment and
conversation remain available. This checkpoint is outside the TestFlight build.

Picker completion, queued claim retirement and abandoned provider cleanup now
announce the existing Root notification topic independently of the original
reply. Module liveness guards late announcements. Status reads and terminal
replays stay silent. The native fixture passes 172 checks, including retirement
ordering, main-thread delivery, shutdown silence and abandonment settlement.
All 3,940 mobile tests, boot and the normal build pass. Independent source review
finds no introduced blocker. The normal app cancels a picker, opens the next
picker, imports and previews the real Markdown file, then removes it and returns
to an empty usable composer. The native NewTask view remains inactive pending
its producer integration. This checkpoint is outside the TestFlight build.

The frozen NewTask owner now continues Photos and Files intake through file
holds, one attachment batch, confirmed saved bytes and immutable finish or cancel.
Historical and accepted files remain held for Undo; rejected issued files release
only after terminal confirmation. Retries preserve the original operation and
never repeat a committed batch. Superseded save and acknowledgment failures no
longer disable another editor. Native dictionary property order does not change
inventory identity. All 3,988 mobile tests, strict TypeScript, boot and the normal
iPhone build pass. Independent review passes all 114 owner cases and finds no
additional blocker. The normal app receives `CONTINUATION_CHECKPOINT_OK` from the
real provider and restores that reply, environment and empty composer after
restart. Native intake is mocked in the owner tests. Explicit resume, history
handback and actual native NewTask acceptance remain open. An uncertain finish
whose saved record changes also needs positive reconciliation before activation.
The textarea remains active. This checkpoint is outside the TestFlight build.

Actual host recreation now passes 50 checks in a focused iPhone fixture using
one real ExactSession and the production T3 factory. A retained predecessor
source makes the successor factory return nil. Acknowledging that receipt and
changing props leaves the refused node intact. Changing the authored singleton
key creates a fresh callback nonce, handle and Ready event. After acknowledgment,
the replacement accepts real UIKit typing and retains its final source through
teardown. This covers host lifecycle; full native NewTask acceptance remains open.

Independent editor teardown now retains file bytes with its pending final source.
Actual UIKit Undo previously restored a file reference whose bytes were deleted
before Root could save that receipt. Detached claims now refuse queued acquire
and picker work while protecting existing bytes until exact acknowledgment.
Mounted frozen acknowledgment keeps its live claim; saved frozen teardown and
ordinary retirement retain their existing cleanup. All 225 native custody checks
pass, including the reproduced failure, save/cleanup retries and old receipt
isolation. The 88 standalone operations and 37 protocol checks, all 3,988 mobile
tests, boot and normal build pass. Independent review finds no introduced defect.
The normal app receives `FINAL_SOURCE_CUSTODY_OK`, imports and previews a real
Markdown file, removes it, and restores the reply, environment and empty composer
after restart. Native NewTask resume and history handback remain open. This
checkpoint is outside the delivered TestFlight build.

Two normal NewTask Files drives return to Choose project after selection or
Cancel. The typed draft survives in Unsent, but the selected file is not retained.
An instrumented Cancel drive returns correctly with the authored route and native
stack unchanged. A later normal drive imports and previews the file, restores it
through Unsent after restart, opens it again, and removes it. Its test text is
cleared. The chooser also transiently reported an offline environment while later
checks found both descriptor addresses healthy and the saved connection Connected.
The cause remains open; no navigation fix has been accepted.

A large-catalogue cold launch exposed a blank transcript, unready Send and a
Hermes allocation failure. Composer preparation copied and fingerprinted command
catalogues for every workspace. It now projects only the exact current workspace,
matching the shared resolver and preserving missing-versus-empty overrides.
Instrumented drives observed one composer call allocating about 101 MB before
this change and a maximum below 14 MB afterward. These are allocation totals
across collections, not retained heap or a general performance benchmark.
An instrumented drive sends to the real provider and displays
`COMPOSER_SCOPE_OK`. After all probes are removed, the normal build opens the
tested conversation on its first tap and restores its transcript, environment
and that reply. A separate 18-character draft also survives another restart
with Send ready. Temporary diagnostics and the unsuccessful
clock-argument experiment were removed. Strict TypeScript, all 3,720 mobile tests
and the boot check pass. The desktop source cap was then the sole cap violation.

Terminal verification opens a real project shell, runs `pwd`, selects one output
line, attaches it to the composer, sends it, and receives the requested provider
reply. The development build now keeps the environment details page open while
its URL changes: navigation uses the stable environment ID, and commands retain
their exact route ownership. Native roundtrip URL edits and a stale-command
regression pass. This later correction is not in the POC TestFlight build.
Initial iPhone Files and source reads now complete without pulling to refresh.
A live runtime probe found offscreen browser and device discovery superseding their
permission reads. Preview discovery now follows the consuming chat and viewer
screens, retaining iPad Files chat when its inspector is supported. The native
app opens the real tree and README source on first entry, then switches to
Markdown preview. Source rows use authored presentation without native cell
separators. File title, subtitle and actions now share the native navigation bar
with the reference editor style. The file route now owns an opaque bar with the
sheet background and no separator. Preview/Source switching retains that bar;
returning through Files restores the thread header. Unwrapped source uses the regular platform
monospace font and reference gutter geometry. Wrapped source uses Menlo and the
reference token traits and insets. One virtualized viewport now pans every line
together while its opaque line-number gutter stays fixed. Its shared width uses
the longest displayed UTF-16 line and a measured platform monospace character,
capped like the reference. Tiny files stay within the viewport; a short wide
file stops at its final character after repeated pans. The normal iPhone
drive verifies this on the large schema file, including newly revealed rows,
wrapping after a pan, a pull to refresh, and opening a second file. Wrapping
remounts the viewer and resets both offsets as the reference does. Markdown file
previews now render closed fences as syntax-colored code cards with independent
horizontal scrolling and native Copy feedback. Code uses the reference card
geometry and scales with the base text size. The normal phone copies the same
344-character Caltrain block as the reference, then copies both Messages README
blocks independently. UIKit word selection copies the selected code word.
Preview/Source switching, pulling to refresh and returning to the filtered Files
list pass. The shared splitter now requires a valid closing fence on its own
line and preserves meaningful trailing blank lines. Plain isolated ATX headings now use the pinned reference sizes, line heights
and strong-text color. Workspace and attachment previews share this projection.
The normal iPhone drive verifies the Caltrain title and Run it metrics, native
heading selection, the unchanged 344-character code copy, and an exact
2,043-character Source copy including its final newline. The temporary paste
draft is cleared and remains empty after ordinary restart. All 3,788 mobile
tests, strict app TypeScript, boot and the normal native build pass. Independent
review found strikethrough and bare-link headings losing formatting; those now
retain the generic reader and their regressions pass. Formatted, nested, setext
and unsafe-context headings retain generic rendering. The normal iPhone also verifies dark appearance at 20-point base text,
heading heights and independent code-card panning. System appearance and
16-point text restore through Settings. The existing sent Markdown attachment
shows the same default heading metrics, copies its original 344-character code
block exactly, switches Source/Preview, and returns to its conversation. Heading
accessibility acceptance remains open. Plain prose now folds soft line breaks
within admitted chunks, including single-line inline code. Unsupported Markdown
keeps its original complete chunk. The normal app verifies workspace and sent
attachment wrapping at 16-point System appearance and 20-point dark appearance.
Code copies remain 344 characters, Source copying matches after the device
tool's final-newline normalization, native word selection copies `measured`, and
code-card panning remains independent. System appearance and 16-point text are
restored. Independent review and fifteen projection probes find no introduced issue. Complex
soft breaks, richer table cells, images and heading spacing remain parity gaps. These
changes are outside maintenance build `20261028202307`.
File previews now render GFM tables with plain text and inline-code cells through
one generated adapter over the existing pinned Remark parser. The adapter parses
complete preview source and requires exact agreement with existing code and prose
records before extracting tables. Unsupported cells, references and footnotes retain their original chunk.
Code identities and raw Source remain unchanged. Both preview owners share the
same table projection and cache behavior. Cells retain the reference 160-point
width, borders, padding and header styling. Numeric cell values use tabular digits.
The normal iPhone verifies the real Caltrain table in workspace and sent attachment
previews at 16-point System appearance and 20-point dark appearance. Code copying
remains exactly 344 characters, and workspace Source copying matches after the
device tool's final-newline normalization. Native selection copies an inline-code
cell word. Three-column and five-column repository tables pan together and stop
at their content edge. A five-column drive exposed sideways movement of surrounding
prose; the corrected outer scroll keeps that prose fixed across repeated pans.
A large-text filename also crossed its cell boundary. Cell text now breaks an
overlong word, and the rebuilt app shows the complete filename inside its cell.
System appearance and 16-point text restore and persist after normal restart.
The mobile suite passes 3,822 tests. The combined parser and minimap run passes 46.
Strict app TypeScript, boot, generated-output verification and the normal native
build pass. Independent
review of the projection and 525 comparisons find no introduced issue. Richer
cell formatting, table accessibility and general Markdown parity remain open.
These changes are outside maintenance build `20261028202307`.
A later ordinary restart once leaves the conversation open after its first
enabled Files tap; the second tap opens Files. Three instrumented and three
normal cold launches then open Files on the first tap. Another normal restart
reproduces the miss while the saved-message ownership warning clears. The second
tap opens Files. The successful traces do not establish the cause. No admission
policy changed, and the temporary native probe is removed. A later cross-boundary
diagnostic build lost its native header and was discarded. That approach reached
the three-round limit and supplies no acceptance evidence. Its four source files
are restored, and the normal header returns. Keep this symptom open.
Unclosed/nested fences and reference links across preview chunks remain open.
The draft file route shares this projection but has no new native acceptance.
Files rows now align their labels beside the icons without
system cell separators. Its Files/project header and bottom search reuse the
existing native coordinators. The focused field and dismiss-button frames match
the reference capture. Native no-results search, directory expansion, result
selection, Preview/Source actions and returning with the query intact pass.
Empty/error states reserve the same toolbar clearance as file rows. Hiding the
header coordinator's unused body button restores the first row's folder glyphs.
The inspector-supported Files/chat branch retains its existing search input.
Source rows carry only source fields and reuse their
highlighted projection while the read is unchanged. The normal iPhone app opens
the real 6,175-line schema file that previously exceeded the 100 ms turn budget.
Read failures show an error and Try again instead of an empty body. All 3,716
mobile tests and strict app TypeScript pass. These phone fixes are not in the POC build.
The Apple host preserves pull-to-refresh for a short file that also scrolls
horizontally. Its new regression assertions pass, and the native short-file drive
and tvOS compile complete. Root build, tests, clippy, formatting, boot and workspace
checks pass; caps then reported only the desktop Contract. The full UIKit
run reports 2,134 passes, 17 failures and four skips. Failures include keyboard
availability, native row alignment, HDR metadata and popover rendering. Source
review finds that the failing fixtures omit refresh handlers or bypass the changed
method. No clean baseline run establishes these failures as pre-existing.
A paired learned route still retains its learned marker.
T3 Connect relay login remains unconfigured in the clone; direct pairing is the
verified connection path.

All six root repository checks pass at the mobile checkpoint. Earlier integrated
checks passed build, tests, Clippy, formatting, boot and workspace compilation,
but caps rejected the inherited 2,994-line desktop Root. The five-member
extraction above resolves that failure. Current caps passes.

The full mobile suite passes 3,940 tests, strict app TypeScript passes, and the
normal native simulator build succeeds. The actual Root event regression fails
with `QueueFull` on the former eager-send action and passes with coalesced text.

The app uses Exact Contract, TypeScript/Hermes, and UIKit, with no React Native
runtime or Metro. Use Bun 1.4.2 and the separate app Cargo workspace. Ordinary
restart tests must launch without Exact agent mode, which keeps connection
credentials and environment metadata in memory.

## Prove the core POC first

The first POC must connect to a real authorized T3 environment, open a project,
create a thread, send a message, display streamed output, stop a response, and
retain the environment and conversation after restart. Keep navigation, keyboard
behavior, and connection errors usable on iPhone. Mock responses do not complete
this POC.

Build only the dependencies needed for that flow first. After the core POC works,
complete the remaining reference behavior and visual parity. iPad and desktop
remain in the full goal. Architecture work must unblock an observed build or app
failure in the current milestone.

## Execution milestones

Nate chose iPhone first, then iPad. Desktop remains in the overall plan.
Keep the complete one-to-one goal across these milestones.

1. **Get the iPhone app running.** Start from mobile checkpoint `250d863` on one
   working branch. Keep its reference pin `365aa87982`. Use the normal mobile
   build from `mobile/README.md` and fix actual build or launch failures.
   Finish with Home open in the simulator and a saved screenshot.
2. **Prove environment connection.** Drive Home, Add environment, connection
   failure, successful connection, and return to Home. Restart the app and
   confirm the environment remains saved.
3. **Finish the main chat flow.** Open a project, create a thread, send a message,
   receive streamed output, stop a response, and reopen the conversation after
   restart. Show the working flow and run its focused tests. This completes the
   core POC and triggers a production build and TestFlight submission.
4. **Close the remaining phone differences.** Use a short list of reference
   screens, flows and states. Match each through the real interface, including
   settings, attachments, terminal, loading, empty, error and offline behavior.
   Finish each slice with comparison screenshots and a verified commit.
5. **Complete iPad and desktop parity.** Repeat the flows on each target and fix
   its layout, navigation, keyboard, accessibility and motion differences.
   Desktop uses a separate reference version and needs its own integration work.
6. **Verify and deliver.** Run the required checks on the integrated code, check
   persistence and performance, and confirm an installable build and tester
   access. Complete the production build and TestFlight checkpoint below.

Mobile selects its own Contract entry. The desktop Root extraction is integrated.
Bring other checkpoint changes
into the working branch when a concrete build or behavior dependency needs them.

## Ship production builds at major checkpoints

Produce a new signed production iOS build and submit it through `eas submit`
after the core POC, phone parity, and iPad parity checkpoints. TestFlight builds
are part of each checkpoint. Routine fixes between checkpoints use the local
development loop.

Reuse the clone's existing native release path, EAS project, signing credentials,
bundle ID, and App Store Connect app. Build the tested commit with a new build
number. Submit that exact artifact using its EAS build ID or IPA path and the
existing store submission profile. Follow the [EAS iOS submission guide](https://docs.expo.dev/submit/ios/).

Record the commit, version and build number, build and submission links, and verified
TestFlight state in the PR. Check Apple processing and intended tester access.
An upload alone does not establish installability. Handle routine release errors
with existing credentials and truthful compliance answers. If a human-only
account action blocks availability, report that action and continue independent
app work. Distribution scope is TestFlight.

## Execute with pstack

Use `pstack:poteto-mode` to select the applicable playbook and handle implementation,
debugging, and review. Follow Nate's configured pstack model roles. Choose the
smallest working change that advances the next app flow.

- Routine source edits, dependency installation, builds, tests, simulator use,
  local commits, normal pushes to PR216, checkpoint production builds, and EAS
  TestFlight submissions are authorized. Make reversible implementation decisions
  without asking Nate to manage the work.
- Start with one implementation owner. Delegate bounded work on disjoint files
  after the iPhone baseline runs. Give shared entry points, build output, and
  each simulator one owner. Review independently at working milestones.
- Build, drive the changed flow, fix observed differences, then commit and push.
  Commit small verified changes frequently, before combining the next slice.
  Use focused checks during development and run the required integrated checks
  before delivery. A passing build alone does not establish parity.
- After three failed fixes for one target, stop that approach and report the
  blocker briefly. Continue independent flows within the existing scope.
  Collect decisions only Nate can make into one handoff after independent work.
- Keep progress in this file and the PR. Report the working flow, remaining
  mismatch, and pushed commit. Keep useful logs and screenshots with the work.
- Finish when the reference flows, visual behavior, persistence, accessibility,
  and performance are verified on iPhone, iPad, and desktop. Record incomplete
  acceptance and any remaining tester access blocker explicitly.

The next native NewTask prerequisite publishes observed text, context and file
inventory together. Focused tests cover retained-file deletion and Undo, cleared
upload state on restoration, duplicate equivalent holds, stale captures,
malformed metadata, preserved images and shelf files, and persistence through an
actual client reload. Evicted context payloads stay unresolved even if their file
holds remain. All 3,995 mobile tests, strict application TypeScript and boot
checks pass. Independent review reports no introduced findings, with 127 focused
tests and 36 additional boundary probes passing. The inactive native NewTask
owner now uses this publisher and implements picker resume and repeated receipt
cycles in commits `85a636b2af` and `88e2c140cf`. Independent review reproduces two
correctness defects: failed rejected-file release loses its retry path after
resume, and a malformed resume proof can skip native event counts. Both are
fixed with failing-before and passing-after regressions. Cleanup remains owned
by the continuing hold ledger; resume requires the exact successor count.
All 208 focused checks and 4,055 mobile tests pass, along with strict TypeScript
and boot. Parent reruns the focused checks and builds the normal iPhone app.
The saved LAN address is no longer reachable after the Mac's address changes; updating it through the app's existing
form to the same environment at loopback restores connection. The normal app
receives `RESUME_PREREQUISITE_OK` from the real provider and restores that reply,
the environment and an empty composer after restart. This drive checks the
existing active flow. Native IO in the new owner tests is mocked. The review's
three redundant comments are removed. Actual native NewTask producer acceptance
remains open. No TestFlight qualification is added.

Native finish now accepts validated known-choice drift without changing the
original immutable publication or replay signature. Every other record field,
unknown choice metadata, opaque option rows, and metadata-bearing option ID and
index remain bound. The existing coordinator still validates actual inventory
and every issued byte before writing current preferences. Baseline native runs
reproduce both delayed-first-arrival failures. The fresh final fixture passes
1,361 checks with real file and PNG bytes. It covers arrival orders, lost replies,
protected-field refusal, save failures, byte mutations, retirement and restart.
All 4,056 mobile tests, strict TypeScript, boot and Foundation protocol/operations
checks pass. Independent review reports no correctness finding, reruns 216
owner/IO tests, and passes 294 additional raw-JSON Foundation probes. Both sets of
135 native build inputs match their hash receipts. The two redundant comments
identified in review are removed. This resolves the known-choice finish gate;
generic metadata mutation still refuses. The normal app build passes its module
roster qualification, receives the real provider reply `FINISH_BINDING_OK`, and
restores that reply, the saved environment and an empty ready composer after
restart. Native NewTask UI and producer acceptance remain incomplete, and the
TestFlight binary is unchanged.
