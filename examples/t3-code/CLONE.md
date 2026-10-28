# T3 Code clone handoff

Updated October 28, 2026, Toronto time. Finish the one-to-one clone with pstack
handling routine execution and review. The legacy scheduled coordinator stays
disabled. Use this handoff when taking over the work.

## Current state

The clone is incomplete. The desktop app is in `examples/t3-code`; the separate
iPhone and iPad app is in `examples/t3-code/mobile`.

These implementation checkpoints are pushed to GitHub:

| Work | Commit | Branch |
| --- | --- | --- |
| Root app modules and compiler fixes | `61b7997` | [Root checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-root-checkpoint-20261028) |
| App refactors and focused tests | `0e6c9ae` | [App checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-contract-apfs-r1-20261024) |
| Mobile implementation and release work | `250d863` | [Mobile checkpoint](https://github.com/ccheever/exact2/tree/t3/clone-draft-apfs-r1-20261024) |

[PR216](https://github.com/ccheever/exact2/pull/216) remains a draft on a divergent
branch. Integrate the checkpoints into the working branch as concrete dependencies
require them.

- The desktop app extraction is unfinished. Current `app.contract` is
  2,994 lines against the repository's 1,500-line rule. Current Contract keeps
  ordinary resources, mutations and tasks at Root. No fitting replacement was
  selected.
- Focused compiler, chat and command tests passed. Whole-app behavior, visual
  parity and final integrated checks remain unverified.
- The InlineShortcut comparison has no complete verdict. Its cached test
  executable is missing, so another test run needs a fresh build.
- Live App Store Connect status on October 28 reports POC build
  `1.0.0 (20261028074348)` as `VALID` and `IN_BETA_TESTING` internally. Its exact
  build is assigned to `Team (Expo)`, with one installed tester and one invited
  tester. External distribution was not extended.

Application tests, technical drafts and local build evidence are retained.
Consult them for specific implementation questions rather than as a task queue.

The current pickup starts at mobile `250d863`. The ordinary iPhone simulator
build pairs with the real local T3 environment. Removing unused per-row Arrange
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
does not qualify cached conversations during a real network outage. Canceling
the system file picker restores an enabled composer.

The normal iPhone build imports a 2 KB Markdown file through Choose Files, opens
its draft preview, and retains the file after restart. Sending uploads the file to
the real environment and receives `ATTACHMENT_POC_OK`. After another restart,
the sent attachment opens from the saved conversation. Inline file-chip taps now
open that same draft preview and Back retains the attachment. Opening first saves
observed text, then rechecks the editor and file; a blocked-save regression covers
rapid typing and route/file changes before navigation. Stale mounts, token ranges,
draft text and removed files cannot open it. Other rich context actions and
new-task photo-library import remain incomplete. Sent Markdown attachments now use the
workspace file code cards: Preview/Source round trips, horizontal code scrolling,
and the exact 344-character code-body copy pass on iPhone. Preview parsing accepts
CRLF/CR while Source retains original newlines. Generic Markdown prose,
tables and images still differ from the reference. These later changes are
outside the POC TestFlight build.

The ordinary thread also imports a 567,032-byte PNG through Files. Its restored
draft opens in Quick Look; sending receives `IMAGE_ATTACHMENT_OK`, and the sent
image opens after restart. Preview resolution now follows image context to its
file-backed bytes; removing the reference or file prevents a new preview.
Ordinary-chat Photo Library selection is now verified in the normal build. The
system picker imports the test PNG as `IMG_0007`; its draft and strip preview
survive restart. Sending it with an explicit message receives `PHOTO_LIBRARY_OK`
from the real provider. After another restart, the saved conversation opens the
sent image correctly. Inline image-chip activation still reaches an unsupported
context action; the attachment strip remains the verified preview path.
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
The inactive native NewTask editor prerequisite reserves eight final-source slots
and retains teardown receipts until explicit acknowledgment. Its actual UIKit
fixture passes 86 checks, including text/chip source recovery, repeated reads,
blocked replacement and retry. The normal app builds and the textarea remains
active. Activation still requires freeze/resume, the client-lifetime Root save
drain, stable draft/incarnation blocking across transport generations, and native
node recreation after a factory refusal. Direct editor-object retry alone does
not qualify recovery through the host. Source review found no introduced blocker
in this prerequisite.
The uncommitted native handoff unit freezes its writer, retains its captured source
until save acknowledgment, and resumes with the producer's explicit document.
Its reservation follows stable draft identity across transport generations. Review
found stale serialized chip offsets after native typing; the current correction
uses the rebased token array for comparison and Undo capture. In the latest UIKit
fixture, chip restoration and producer Undo pass, but an unchanged-resume Undo
assertion fails. The result is 113 passed and one failed. This reached the
three-round fix limit. The unit remains inactive and uncommitted. Root drain,
producer integration and native-node recreation also gate activation.

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
clock-argument experiment were removed. Strict TypeScript, all 3,717 mobile tests
and the boot check pass. The desktop source cap remains the sole cap violation.

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
line and preserves meaningful trailing blank lines. Prose still uses portable
Markdown; heading sizes, soft line breaks, tables and images remain parity gaps.
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
checks pass; caps still reports only the desktop Contract above. The full UIKit
run reports 2,134 passes, 17 failures and four skips. Failures include keyboard
availability, native row alignment, HDR metadata and popover rendering. Source
review finds that the failing fixtures omit refresh handlers or bypass the changed
method. No clean baseline run establishes these failures as pre-existing.
A paired learned route still retains its learned marker.
T3 Connect relay login remains unconfigured in the clone; direct pairing is the
verified connection path.

All six root repository checks pass at the mobile checkpoint. On the integrated
PR, build, tests, Clippy, formatting, boot and workspace checks pass. Caps finds
one inherited violation: desktop `app.contract` is 2,994 lines against 1,500,
unchanged from the original PR head. A bounded extraction attempt made no changes;
the desktop checkpoint needs broader integration. Keep the failure visible.

The full mobile suite passes 3,716 tests, strict app TypeScript passes, and the
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

Mobile selects its own Contract entry. The unfinished desktop Root extraction
does not inherently block the first mobile build. Bring other checkpoint changes
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
