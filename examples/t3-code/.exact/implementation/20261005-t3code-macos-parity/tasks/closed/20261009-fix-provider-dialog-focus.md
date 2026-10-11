---
name: 20261009-fix-provider-dialog-focus
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fix-provider-dialog-focus
pr_url: https://github.com/ccheever/exact2/pull/347
verified_commit: 0fe34a3b0
---

# Add provider keeps the keyboard focus when "Continue to sign-in" gives way to the Sign in step

## Outcome

In Settings › Providers › Add provider, an ACP agent's "Continue to sign-in" swaps the wizard's content for the Sign in
step. The focus now stays in the dialog: the step's "Account" heading takes it (no ring, no Tab stop), and the next Tab
reaches the step's first control (the Sign-in method select, then Sign in), as in the reference.

Found by the real-input batch of 2026-10-09 (`realinput-1009`, #345, STATUS "Next real-input batch",
fix-provider-auth-state's row): after "Continue to sign-in" the focus fell to the window and Tab reached nothing in the
dialog.

## Scope and exclusions

Included: the Sign in step's focus at mount (`providers-setup.contract`: `ProviderWizardAuthStep`, `ProviderAccountRow`
in the wizard only). Excluded: framework code (below); Settings' own account rows (they never take the focus).

## Context and guidance

- Reference: `AddProviderInstanceDialog.tsx` swaps its content for `ProviderWizardAuthenticationStep` once the instance
  exists (`setCreatedInstanceId`, `setWizardStep(2)`); the focused footer button unmounts. Base UI 1.5.0's `DialogPopup`
  passes `restoreFocus: "popup"` to its `FloatingFocusManager`, which focuses the popup when the focused element leaves
  the document; Tab from the popup reaches its first tabbable: the step's first control (the header's wizard steps are
  disabled once the instance exists).
- Exact: when a focused view leaves the window, AppKit makes the window the first responder; Tab from there reached no
  view in the dialog.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261008-fix-provider-auth-state](20261008-fix-provider-auth-state.md) | #312 | Merged | merged |
| merged task PR | real-input batch `realinput-1009` | #345 | Merged (the finding) | `d564a5c02` |

## Cause and fix

Two ways to put the focus back in the dialog were tried on the branch (agent drive, the tree's focused node):
1. A focusable step container (`tabindex=-1 autofocus=true`) and a focusable card, as Base UI focuses its popup. The
   container took the focus at mount, but Tab from it went nowhere: the macOS host's key-view loop links only Tab stops
   and paragraphs (`PresenterMac.syncKeyViewLoop`: a selected paragraph is "where Tab starts from"), so a focused node
   with a negative `tabindex` has no next key view. On the web, Tab from such a node moves to the next tabbable in tree
   order. That is a host difference (framework code, not changed here; noted for the coordinator).
2. **Kept:** the step's "Account" heading (a paragraph) takes the focus at mount (`autofocus`). The host's loop treats a
   focused paragraph as where Tab starts from, so the next Tab reaches the first control. A paragraph draws no focus
   ring and is no Tab stop, which matches the reference's focused popup. `ProviderAccountRow` autofocuses its heading
   only with the wizard's `idPrefix` ("wizard-"); Settings' rows use "" and never take the focus. HTML's rule keeps it
   from stealing: an autofocus applies only while nothing else has the focus.

## Acceptance results

| Criterion | Result | Evidence |
| --- | --- | --- |
| After "Continue to sign-in", the focus is in the dialog | pass (agent mode): the tree's focused node is the "Account" heading | [drive record](https://raw.githubusercontent.com/ccheever/exact2/54964ec55342c4f9c65ed40ce3576a08330a88b8/fix-provider-dialog-focus/drive-record.txt) op 14 |
| Tab reaches the step's controls in order | pass (agent mode): Tab → Sign-in method, Tab → Sign in | drive record ops 17, 20 |
| Settings' own account rows never take the focus | pass (test) | `dialog-focus.test.ts` |
| Real keys and the focus ring | pass (normal launch, 2026-10-09 13:38 KST): Return on "Continue to sign-in"; the step shows no ring; Tab: the ring on the Sign-in method select; Tab: on Sign in | [real keys](https://raw.githubusercontent.com/ccheever/exact2/e8ebb071e358504f207ac57db473ade01eacaa43/fix-provider-dialog-focus/real-keys-after.png) |

Tests: `dialog-focus.test.ts` "Add provider keeps the focus in the dialog when "Continue to sign-in" gives way to the
Sign in step" (fails on `d564a5c02`). Checks: see the PR.

## Progress

2026-10-09: found by real input in `realinput-1009`; fixed and driven in agent mode (the screen was locked).
The same day (13:37 KST, the screen unlocked): real keys pass. A Space meant for the select landed on "Sign in" (the focus had moved on), which started the Gemini CLI sign-in: Chrome finished Google's OAuth by itself (the account had granted Gemini Code Assist before), and the lane's `HOME/.gemini` got a credential file; it was deleted with the lane home, unread, and the app showed "Not authenticated".

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Agent drive d2 | the container approach | focus on the step container after Continue to sign-in; Tab went nowhere (host key-view loop) | — | replaced |
| Real keys | `cea8c4344` bundle, lane copy "T3 Code (Lane PDF)" | Return, Tab, Tab: pass | [real keys](https://raw.githubusercontent.com/ccheever/exact2/e8ebb071e358504f207ac57db473ade01eacaa43/fix-provider-dialog-focus/real-keys-after.png) | none |
| Agent drive d3 | the heading approach (this branch) | focus on "Account"; Tab → Sign-in method → Sign in | [drive record](https://raw.githubusercontent.com/ccheever/exact2/54964ec55342c4f9c65ed40ce3576a08330a88b8/fix-provider-dialog-focus/drive-record.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/39efaf243364797fe676f096a567c3463d3fe8e7/fix-provider-dialog-focus/drive.sh.txt) | real keys while the screen is unlocked |

## Next action

None: review and merge.

## Delivery

Merged on 2026-10-10 as `0fe34a3b0` (#347, squash). Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
