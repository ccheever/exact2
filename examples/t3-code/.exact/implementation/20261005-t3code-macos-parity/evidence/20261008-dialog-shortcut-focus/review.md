# Independent review, 2026-10-08

Reviewer: a separate agent. It was given the uncommitted change on `d82fb6a47`, the task record, the
reference (T3 Code 1e2ecbd975: ConfirmDialogHost, CustomSnoozeDialog, ui/dialog, ConnectionsSettings),
the host code the change relies on and the agent drive transcript (`agent-focus-drives.txt`). It was not
told an expected verdict and changed no file. It confirmed in the host code the framework facts the change
relies on (the macOS key-view loop skips inert, hidden and negative-`tabindex` nodes; `key` handlers bubble
and `preventDefault()` stops Tab's default; shortcut and hatch presses ignore `tabindex`; `autofocus` waits
while another control holds the focus; `focus(id)` runs after the batch mounts and needs a non-zero size;
action reads see the starting state).

## Round 1: FAIL

| # | Finding | Severity | Resolution |
| --- | --- | --- | --- |
| B1 | Making Settings `inert` under `restEditor`/`snapshotSetupOpen` (a) froze the page when `restEditor == "task"` but no editor renders (a deep link to a missing task, a load error, while loading), and (b) dropped the opener's focus, so Escape no longer returned it (scheduled task, project icon/file, remove project/checkout, project action, SnapShot setup) | blocking | Reverted; those dialogs keep the base behavior, recorded under X53 |
| 1 | The `inert` comment overstated its coverage (`connectionRemove`, `saOpen`, `archiveConfirm` dialogs still leave Settings interactive) | non-blocking | Gone with B1's revert; the remainder is in the task record's not-done list (X53) |
| 2 | AppConfirm takes no focus when a still-focused button opens it (terminal tab X and trash, composer image remove) | non-blocking | The terminal's two buttons now move the focus into the confirm (`askClose`; Cancel already returns it to the terminal). The image-remove and right-panel openers are left as they are: they have no focus return, so moving the focus in would regress them (X53) |
| 3 | Two AppConfirms mounted at once share ids | non-blocking | `confirmId` per call site (`app-confirm`, `server-update-confirm`, `provider-setup-confirm`, `wizard-sign-out-confirm`); testIds unchanged |
| 4 | `r4-surfaces.contract`'s 0×0 pressable `tab-cancel` anchor is still a Tab stop; the test only scanned `button … width=1 height=1` | non-blocking | `tabindex=-1` on it; the test scans any pressable of size 0 or 1 |
| 5 | `dialogReturn` ignores the opener (a ⌘-chord typed in the composer returns to the row; the title-menu route once added) | non-blocking | Recorded as an approximation: the clone has no handle on the previous focus (X53) |
| 6 | `app.contract` is at 1,500/1,500; the sibling `title-custom-snooze` task will need room | non-blocking | Noted in the PR for the coordinator |
| 7 | `dialog-focus.test.ts` matches source text; its header claimed hosts follow tree order although macOS skips date/time/select | non-blocking | Header corrected (X52 named; the file guards wiring, the drives prove behavior); `sidebar.test.ts`'s `dialogReturn` test exercises behavior |
| 8 | Real-keyboard evidence and the records were missing | non-blocking | Records updated; real keys deferred to the real-input batch (screen locked) |

The reviewer ran `bun test examples/t3-code` (2520 pass, 1 skip, 0 fail), `contract build` and `caps`; the
transcripts matched acceptance rows 1–5 for the dialogs they covered.

## Round 2: PASS

Given the repairs above and transcript section 10 (the rebuilt app's drives). It checked that
`app-settings.contract` differs from the base only by the `closeDialog=` rename, that the terminal's
`askClose` actions return through `confirmUi`, that every AppConfirm call site passes a `confirmId` and the
three `focus("app-confirm-cancel")` callers target the main window's, and that a wider scan (any `button`,
any `key`/`focus`/`blur` node of size 0 or 1) finds no offender. It re-ran `contract build` and
`bun test examples/t3-code` (2519 pass, 1 skip, 0 fail). Verdict: PASS, no blocking findings. Open and
recorded: the real-keyboard batch, the row as the sidebar's return target (X53), the line cap.
