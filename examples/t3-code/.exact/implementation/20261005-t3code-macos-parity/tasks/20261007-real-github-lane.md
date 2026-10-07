---
name: 20261007-real-github-lane
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-real-github-lane
pr_url: https://github.com/ccheever/exact2/pull/233
verified_commit: null
---

# A real-GitHub lane (sandbox, seed, probe) replaces the fake gh; the pull request rows already built are re-verified live

## Outcome

The user's decision (2026-10-07): "The work that used a fake GitHub now connects to the real GitHub."
Pull request features are verified against real GitHub through the real GitHub CLI that the T3
server spawns, signed in by the user personally into lane-only config dirs. This task replaces
[20261005-fake-github-fixture](20261005-fake-github-fixture.md) (now `blocked`, superseded) with:

1. a lane recipe: isolated T3 servers whose `gh` is the real CLI on a lane config dir, two accounts;
2. a disposable sandbox repository the user approves, and an idempotent seed that fills it with
   what the pull request features need, through the lane `gh` only;
3. an RPC probe that sends every pull request RPC of the old verb table to a lane server against
   the sandbox, and reads each write back from GitHub;
4. one live macOS drive re-verifying the pull request behaviors already built that were verified
   only against the fake `gh`;
5. the plan's records moved off the fake `gh`.

Later pull request tasks reuse the shared login and the sandbox without asking the user again.

## Scope and exclusions

Included: `examples/t3-code/tools/github-lane/` (`lib.mjs`, `lane.mjs`, `seed.mjs`, `probe.mjs`,
`lane.test.ts`, `README.md`); `pr-profiles-injection.test.ts` (the states real GitHub cannot
produce); the live drive; fixes to clone bugs the drive finds (three rounds at most); records.

Excluded: other hosts' CLIs (U21: capability-driven unit tests only); building the six pull request
tasks' features; workflow approval, which these two accounts cannot produce (the probe row names
why); any repository other than the sandbox and the second account's fork.

The apparatus is approved by the user's request to use real GitHub ("keep it small").

## Context and guidance

Parent specification: [spec](../spec.md). Reference server: T3 Code `1e2ecbd975`
`apps/server/src/pullRequest/GitHubPullRequestCli.ts`, `githubStackActions.ts`,
`gitHubPullRequestJson.ts`, `GitHubPullRequestProvider.ts` (`gitHubViewerPermissions`). The lane
server is the official release the app embeds (`server-runtime/runtime-pin.json`,
`0.0.46-nightly.20261005.2667`, four commits after the pin), staged by `stage-runtime.mjs`.

What the server does with `gh` (read at the pin): it rebuilds `PATH` from `zsh -ilc`
(`os-jank.ts` `hydratePosixPath`, login shell first, then the inherited `PATH`), runs
`gh auth token --hostname github.com` in the project's checkout, and passes that token to every
later call as `GH_TOKEN`/`GITHUB_TOKEN` (`GitHubCli.ts` `executeRaw`). The lane therefore sets
`GH_CONFIG_DIR` three times (server environment, isolated `.zprofile`, the wrapper) and the
wrapper refuses to run unless that dir holds its own token, so the keyring is never read.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | Real GitHub instead of the fake `gh` (user, 2026-10-07) | none | — | decided |
| recorded decision | Two accounts: the user signs in a second account too (user, 2026-10-07) | none | — | decided |
| recorded decision | The sandbox: `daehyeonmun2021/playground`, public, neutral playground content (user, 2026-10-07) | none | — | decided; created by the seed |
| user action | The user signs both accounts in to the lane config dirs | none | `gh api user -q .login`: primary `daehyeonmun2021`, second `daehyeon-mun` | done 2026-10-07 |
| merged task PR | [20261005-embedded-server-runtime](closed/20261005-embedded-server-runtime.md) | #222 | the staged release runs as the lane server | merged |

## Implementation notes

**Lane recipe** (`tools/github-lane/README.md`; paths and variables only). Shared dir
`T3_GITHUB_LANE_SHARED` = the base checkout's `target/t3-ui-parity/github-lane`: `gh/` (primary),
`gh-second/` (second), `sandbox.json`. Per checkout `T3_GITHUB_LANE` = `target/github-lane`:
`bin/<account>/gh` (wrapper), `home/<account>` (`.zprofile` puts the wrapper first and sets
`GH_CONFIG_DIR`; `.gitconfig` resets every credential helper and uses `gh auth git-credential` of
the lane gh), `servers/<account>` (T3 home, Codex, Claude, XDG, tmp, the sandbox clone), `logs/`.
Server environment built from nothing: `T3CODE_TELEMETRY_ENABLED=false`, `GIT_CONFIG_NOSYSTEM=1`,
ports 16520 (primary) and 16521 (second). Sign-in uses `--insecure-storage`, which keeps the token
in the lane dir's `hosts.yml`; the keychain default would overwrite the machine's own `gh` item.

Checked before the login (2026-10-07): the login shell of a lane server resolves
`bin/primary/gh` and the primary lane config dir (`lane.mjs which`); a lane server with a scratch
project spawned `gh auth token --hostname github.com` and `gh api graphql …` through the wrapper
with `GH_CONFIG_DIR` = the lane dir, and the wrapper refused them (no token yet), so nothing
reached GitHub.

**The repository** (user decision 2026-10-07): `daehyeonmun2021/playground`, public, owned by the
primary account (`daehyeonmun2021`); the second account (`daehyeon-mun`) is its collaborator
(write; a personal repository has no other role) and owns the fork `daehyeon-mun/playground`. It is
a plain playground: no code from exact2 or T3 Code, and no "t3", "T3 Code", "sandbox", "exact" or
clone wording in the name, description, README, branches, titles, bodies, labels or messages
(`lane.test.ts` checks every string the seed writes). main requires the `ci/build` status, admins
included, so merges wait for it and auto-merge can be armed. Checks are commit statuses; there is
no workflow.

**Seed** (`seed.mjs`). Base history of seven commits with fixed contents, author and dates (the same
SHAs in every checkout); branch protection; labels; the second account as collaborator (the primary
invites, the second accepts that invitation by its id through its own lane gh, so its account-wide
invitations are never listed) and its fork; pull requests (numbers in `sandbox.json`): `open-clean`
(`feature/changelog`, `ci/build` success, labels, review requested from the second account),
`draft`, `closed`, `merged`, `conflict` (branched before main changed the same line), `failing`
(`ci/test` failure), `running` (`ci/build` pending), `behind` (branched three commits back),
`many-files` (310 files under `data/`, so GitHub answers 406 to `pr diff` and the server pages the
files API 100 at a time), `second-review` (authored by the second account; the primary's dismissed
request-changes review with four line threads: unresolved, resolved, outdated by a follow-up
commit, and a long one that pages; a standing request-changes review; comments from both;
reactions), `primary-review` (approved and commented by the second account), `cross-repo` (from the
fork), a two-layer GitHub stack (`stack-bottom`, `stack-top`, stack 120), and 105 open
`chore/note-NNN` pull requests (the list pages past its 99-row slice). Runs: the first created the
repository and stopped at note 20 on a dropped connection (the call had landed; retries now treat
"already exists" as done); the second finished (#1–#117); the third created nothing (idempotent);
the fourth added the stack (#118, #119, stack 120); the fifth, after the drive, restored the draft
(#107 back to draft) and opened `open-clean` generation 2 (#132) beside the merged #106. A retried
reply on a dropped connection left the long thread with 13 comments instead of 12.

**Probe** (`probe.mjs`). Per RPC: the decoded summary or the typed failure, the `gh` calls the
server made for it (from the wrapper log), and for writes a read-back from GitHub with the lane
gh. Writes go to the probe's own pull requests (`chore/check-<run>-<n>`, label `chore`) and stack.
Profiles: one server per account, `pullRequests.detail` as admin author, admin reviewer, write
reviewer, write author and cross-repository author.

**Unit fallback** (`pr-profiles-injection.test.ts`, 12 tests). Profiles two accounts cannot hold
on a personal repository (read, triage, a read-only fork author, maintain) through the server's
`gitHubViewerPermissions` into the clone's `presentDetail` and `rowAction`; injected failures (detail
failure and not-found, activity failure, refused write, candidate failure) and delays (the panel's
loading state until the answer lands; the row's 30 s retry; a read let go mid-flight). `lane.test.ts`
(12 tests) covers the wrapper's refusal and pass-through, the server environment, the login shell's
PATH, the seed's deterministic history, state rules and neutral wording, and the probe's record.

**Clone fix** (`r6-pr-actions.ts` `readDetail`; found by the live drive). The details card's
pull request row reads the detail and checks for the thread's link. When Exact replaced the card's
answer while GitHub was still answering (about 0.8 s here, while the server's link syncs bumped the
revision), the read ended as "let go" and was stored as a finished read with no detail and no error;
every later answer treated that as fresh, so the row never split into its state, checks and action.
A let-go read now settles nothing (the previous entry is put back) and rethrows, as the other reads
of the clone do; the next answer reads again. The fake `gh` answered instantly, so this never showed.
The reference (React Query) has no such state. Regression test: "the row: a read let go mid-flight
… settles nothing, and the next answer reads again".

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| The server's `gh` is the lane's | Lane server, shared login | `bun lane.mjs which primary`; probe `S-*` rows and `ghConfigs` | the wrapper and the lane config dirs only | macOS | probe JSON |
| Seed | Approved repository | `bun seed.mjs` twice | the second run creates nothing | macOS | seed output |
| Every read decodes | Seeded repository | `bun probe.mjs` | every `R*`, `D-*`, `P*` row decoded | macOS | probe table |
| Every write has its effect on GitHub | Same | `bun probe.mjs` | every `W*` row confirmed by read-back, or not producible with its reason | macOS | probe table |
| States GitHub cannot produce | — | `bun test examples/t3-code/pr-profiles-injection.test.ts` | pass | — | test log |
| Built pull request behaviors, live | Lane server on the repository, the branch's app (agent mode) | one macOS drive and one retry | each behaves as the reference; fixes with before/after pairs | macOS | drive record |
| Records | — | — | fake-github-fixture blocked and superseded; six PR tasks depend on this task; plan, spec, STATUS, AGENT-HANDOFF updated | — | diff |
| Gates | `git add -A` | clone checks, `bun scripts/caps.mjs`, the five checks | pass | macOS | numbers below |

## Results

**Logins** (checked with the lane gh before any write): primary `daehyeonmun2021`, second
`daehyeon-mun`; scopes gist, read:org, repo, workflow; tokens in the lane dirs' `hosts.yml` only.

**Probe.** Three runs. Run 1 (13:57 UTC): 77 of 77 rows decoded, every write confirmed (its R13
read-back expected 12 thread comments where GitHub had 13; the check now reads GitHub's count).
Run 2 stopped at R12 on a dropped connection. Run 3 (14:40 UTC, the table below): 71 of 77 decoded,
0 read-back mismatches; its six failures were "GitHub CLI command failed" / "read: operation timed
out" on this Mac's connection to api.github.com that evening, and each of those rows passed in run 1
(shown from run 1 and marked). Every `gh` call came from the lane wrappers with the lane config dirs
(`ghConfigs`), 281 calls in run 1. GitHub stacks are available to this account
(`GET repos/…/stacks` 200), so the stack read, stack rebase and whole-stack merge are real rows.
Workflow approval decodes but has nothing to approve: the playground has no workflow, and GitHub
asks approval only for outside or first-time contributors' fork runs, while the second account is a
collaborator.

| # | RPC | Result | `gh` calls the server made | GitHub read back |
| --- | --- | --- | --- | --- |
| S-primary | lane server | decoded: 0.0.46-nightly.20261005.2667 on 16520; login shell resolves gh to the lane wrapper: true (0 ms) |  |  |
| S-second | lane server | decoded: 0.0.46-nightly.20261005.2667 on 16521; login shell resolves gh to the lane wrapper: true (0 ms) |  |  |
| R1 | pullRequests.routingIdentity | decoded: viewer daehyeonmun2021 (384 ms) | auth token --hostname github.com; api user github.com | confirmed: lane gh user = daehyeonmun2021 |
| R2 | pullRequests.routing | decoded: github daehyeonmun2021 playground (47 ms) | auth token --hostname github.com |  |
| R3 | pullRequests.list (open, 10 a slice) | decoded: 10 rows, truncated true, cursors 1, viewer daehyeonmun2021 (1268 ms) | auth token --hostname github.com; graphql (stdin) |  |
| R4 | pullRequests.list (cursors: next slice) | decoded: 9 rows, none repeated: true (1095 ms) | graphql (stdin) |  |
| R5 | pullRequests.list (open, the page's 99) | decoded: 99 rows, truncated true (4284 ms) | graphql (stdin) |  |
| R6 | pullRequests.list (search) | decoded: 2 rows: #132 #106 (878 ms) | graphql (stdin) | confirmed: #132 found |
| R7 | pullRequests.listStats | decoded: 10 stats (832 ms) | graphql s0: repository |  |
| D-open-clean | pullRequests.detail (open-clean #132) | decoded: open, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (838 ms) | graphql viewerCanUpdateBranch |  |
| D-draft | pullRequests.detail (draft #107) | decoded: draft, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (662 ms) | graphql viewerCanUpdateBranch |  |
| D-closed | pullRequests.detail (closed #108) | decoded: closed, mergeable, base unknown, 0 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen] verdicts [comment] (715 ms) | graphql viewerCanUpdateBranch |  |
| D-merged | pullRequests.detail (merged #109) | decoded: merged, unknown, base unknown, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen] verdicts [comment] (833 ms) | graphql viewerCanUpdateBranch |  |
| D-conflict | pullRequests.detail (conflict #110) | decoded: open, conflicting, base behind by 11, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (1048 ms) | graphql viewerCanUpdateBranch |  |
| D-failing | pullRequests.detail (failing #111) | decoded: open, mergeable, base behind by 9, 2 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (735 ms) | graphql viewerCanUpdateBranch |  |
| D-running | pullRequests.detail (running #112) | decoded: open, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (966 ms) | graphql viewerCanUpdateBranch |  |
| D-behind | pullRequests.detail (behind #113) | decoded: open, mergeable, base behind by 12, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (812 ms) | graphql viewerCanUpdateBranch |  |
| D-many-files | pullRequests.detail (many-files #114) | decoded: open, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (1069 ms) | graphql viewerCanUpdateBranch |  |
| D-second-review | pullRequests.detail (second-review #115) | decoded: open, mergeable, base behind by 9, 0 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment approve request-changes] (771 ms) | graphql viewerCanUpdateBranch |  |
| D-primary-review | pullRequests.detail (primary-review #116) | decoded: open, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (828 ms) | graphql viewerCanUpdateBranch |  |
| D-cross-repo | pullRequests.detail (cross-repo #117) | decoded: open, mergeable, base behind by 9, 0 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment approve request-changes] (1769 ms) | graphql viewerCanUpdateBranch; run list --commit 4bdd36b4b54b3a716bccaf945e80f3c1f78351cd --branch do; graphql query; pr list --state open --head docs/contributing --limit 1001 --json numb |  |
| D-stack-bottom | pullRequests.detail (stack-bottom #118) | decoded: open, mergeable, base behind by 9, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] (933 ms) | graphql viewerCanUpdateBranch |  |
| D-stack-top | pullRequests.detail (stack-top #119) | decoded: open, mergeable, base up-to-date, 1 checks, head repo -, actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen] verdicts [comment] (795 ms) | graphql viewerCanUpdateBranch |  |
| R8 | pullRequests.summary | decoded: open Add a changelog (1 ms) |  |  |
| R9 | pullRequests.preview | decoded: Speed up the text helpers open (1 ms) |  |  |
| R10 | pullRequests.checks (failing) | decoded: ci/test:failure ci/build:success (753 ms) | graphql viewerCanUpdateBranch | confirmed: GitHub: ci/build:success ci/test:failure |
| R11 | pullRequests.checks (running) (run 1; run 3: transient "PullRequestOperationError: GitHub CLI command failed.") | decoded: ci/build:pending (722 ms) | graphql viewerCanUpdateBranch |  |
| R12 | pullRequests.activity (reviewed) | decoded: 17 remarks (review-comment,review,issue-comment), 4 threads (resolved 1, outdated 1, paged 1), 2 commits, reviewers daehyeonmun2021, PR reactions 1 (935 ms) | pr view 115 --json author,comments,reviews,commits; graphql reviewThreads |  |
| R13 | pullRequests.threadComments | decoded: 3 more comments, next none (705 ms) | graphql node(id | confirmed: 10 + 3 of 13 comments on GitHub |
| R14 | pullRequests.stack (not stacked) | decoded: null (no stack) (455 ms) | api repos/O/R/stacks?pull_request=132 |  |
| R14b | pullRequests.stack (seeded stack) | decoded: stack 120 on main: #118 feature/greeting-options open, #119 docs/greeting-options open (786 ms) | api repos/O/R/stacks?pull_request=118; api repos/O/R/stacks/120 | confirmed: GitHub stack 120 |
| R15 | pullRequests.linkedThreads | decoded: 0 threads (2 ms) |  |  |
| R16 | pullRequests.reviewerCandidates | decoded: daehyeon-mun* (1138 ms) | graphql viewerDidAuthor; graphql assignableUsers |  |
| R17 | pullRequests.labelCandidates | decoded: 14 labels, applied area:ui,enhancement (1026 ms) | graphql viewerDidAuthor; graphql labels(first |  |
| R18 | POST /api/pull-requests/diff (310 files, every slice) | decoded: 4 slices, 310 files (3129 ms) | pr diff 114 --color never; api repos/O/R/pulls/114/files?per_page=100&page=1; graphql stackEntry; api repos/O/R/pulls/114/files?per_page=100&page=2; api repos/O/R/pulls/114/files?per_page=100&page=3; api repos/O/R/pulls/114/files?per_page=100&page=4 | confirmed: GitHub changed_files 310 |
| R19 | POST /api/pull-requests/diff (open-clean, one read) | decoded: 2 files, next none (924 ms) | pr diff 132 --color never |  |
| R20 | pullRequests.diffFileContents | decoded: old 60 chars, new 65 chars (1177 ms) | api repos/O/R/pulls/116 [.base.sha, .head.sha]; api Accept: application/vnd.github.raw+json repos/O/R/contents/docs/us; api Accept: application/vnd.github.raw+json repos/O/R/contents/docs/us |  |
| R21 | pullRequests.filesViewed | decoded: 2 files with a state, truncated false (524 ms) | graphql files(first |  |
| R22 | pullRequests.invalidate (one, then all) | decoded: void (3 ms) |  |  |
| R23 | pullRequests.subscribeRefreshes | decoded: first value 5, interrupted (1 ms) |  |  |
| P1 | pullRequests.detail as admin, author (run 1; run 3: transient "PullRequestOperationError: GitHub CLI command failed.") | decoded: actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] resolve true labels true reviewers true (697 ms) | auth token --hostname github.com; graphql viewerCanUpdateBranch |  |
| P2 | pullRequests.detail as admin, reviewer | decoded: actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment approve request-changes] resolve true labels true reviewers true (5 ms) |  |  |
| P3 | pullRequests.detail as write, reviewer | decoded: actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment approve request-changes] resolve true labels true reviewers true (1047 ms) | graphql viewerCanUpdateBranch; auth token --hostname github.com; api user github.com |  |
| P4 | pullRequests.detail as write, author | decoded: actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] resolve true labels true reviewers true (957 ms) | graphql viewerCanUpdateBranch |  |
| P5 | pullRequests.detail as write, cross-repository author | decoded: actions [merge enable-auto-merge disable-auto-merge revert approve-workflows ready draft close reopen update-branch] verdicts [comment] resolve true labels true reviewers true (1821 ms) | graphql viewerCanUpdateBranch; graphql query; run list --commit 4bdd36b4b54b3a716bccaf945e80f3c1f78351cd --branch do; pr list --state open --head docs/contributing --limit 1001 --json numb |  |
| W1 | pullRequests.comment | decoded (1778 ms) | graphql viewerDidAuthor; pr comment 136 --body-file - | confirmed: 1 comments on #136 |
| W2 | pullRequests.updateComment (issue comment) | decoded (1935 ms) | graphql node(id; graphql (stdin) | confirmed: body now "Thanks, this reads well (edited)." |
| W3 | pullRequests.setReaction (comment, on) | decoded (1226 ms) | graphql node(id; graphql (stdin) | confirmed: +1 count 1 |
| W4 | pullRequests.setReaction (pull request, on then off) | decoded (3736 ms) | graphql pullRequest(number; graphql (stdin); api repos/O/R/issues/136/reactions | confirmed: heart after on: true, after off: false |
| W5 | pullRequests.update (title and body) | decoded (1190 ms) | graphql (stdin) | confirmed: title "Add check note 3 (renamed)" |
| W6 | pullRequests.setLabels (apply, remove) | decoded (5685 ms) | graphql viewerDidAuthor; api POST repos/O/R/issues/136/labels; api repos/O/R/pulls/136; api DELETE repos/O/R/issues/136/labels/priority%3Ahigh | confirmed: label on: true, removed: true |
| W7 | pullRequests.requestReviewers (ask, take back) (run 1; run 3: transient "gh api repos/daehyeonmun2021/playground/pulls/136: HTTP 0: n") | decoded (5698 ms) | graphql viewerDidAuthor; api POST repos/O/R/pulls/124/requested_reviewers; api repos/O/R/pulls/124; api DELETE repos/O/R/pulls/124/requested_reviewers | confirmed: daehyeon-mun requested: true, then removed: true |
| W8 | pullRequests.submitReview (own: comment, 2 line comments) | decoded (2405 ms) | graphql viewerDidAuthor; api POST repos/O/R/pulls/136/reviews | confirmed: review COMMENTED, line comments at 2,5 |
| W9 | pullRequests.replyToThread | decoded (2169 ms) | graphql viewerDidAuthor; graphql (stdin) | confirmed: reply in_reply_to 4208309501 |
| W10 | pullRequests.setThreadResolution (resolve) | decoded (1949 ms) | graphql viewerDidAuthor; graphql (stdin) | confirmed: isResolved true |
| W11 | pullRequests.setThreadResolution (unresolve) | decoded (1135 ms) | graphql viewerDidAuthor; graphql (stdin) | confirmed: isResolved false |
| W12 | pullRequests.updateComment (line comment) | decoded (1483 ms) | graphql node(id; graphql (stdin) | confirmed: body "Small note on line 2 (edited)." |
| W13 | pullRequests.setFilesViewed (viewed) | decoded (524 ms) | graphql (stdin) | confirmed: viewerViewedState VIEWED |
| W14 | pullRequests.filesViewed (after) | decoded: check-10071440-3.md:viewed (496 ms) | graphql files(first; graphql stackEntry |  |
| W15 | pullRequests.setFilesViewed (unviewed) (run 1; run 3: transient "PullRequestOperationError: GitHub CLI command failed.") | decoded (626 ms) | graphql (stdin) | confirmed: viewerViewedState UNVIEWED |
| W16 | pullRequests.runAction close | decoded (2121 ms) | graphql viewerDidAuthor; pr close 136 --repo github.com/O/R | confirmed: state closed |
| W17 | pullRequests.runAction reopen | decoded (2759 ms) | graphql viewerDidAuthor; pr reopen 136 --repo github.com/O/R | confirmed: state open |
| W18 | pullRequests.runAction draft | decoded (1720 ms) | graphql viewerDidAuthor; pr ready 136 --undo | confirmed: state draft |
| W19 | pullRequests.runAction ready | decoded (1683 ms) | graphql viewerDidAuthor; pr ready 136 --repo github.com/O/R | confirmed: state open |
| W20 | pullRequests.runAction merge (merge) | decoded (10258 ms) | api repos/O/R/pulls/136; api -X POST repos/O/R/statuses/282af2b571e52ae4dcb88b93e5ea3208a7fa1ab; graphql viewerCanUpdateBranch; graphql viewerDidAuthor; pr merge 136 --merge; graphql stackEntry | confirmed: merged true, merge commit parents 2 |
| W21 | pullRequests.runAction merge (squash) | decoded (9102 ms) | graphql stackEntry; graphql viewerDidAuthor; pr merge 137 --squash | confirmed: merged true, parents 1 |
| W22 | pullRequests.runAction merge (rebase) | decoded (7723 ms) | graphql viewerDidAuthor; pr merge 138 --rebase; graphql stackEntry | confirmed: merged true |
| W23 | pullRequests.runAction update-branch (merge) | decoded (7320 ms) | graphql viewerCanUpdateBranch; graphql viewerDidAuthor; pr update-branch 139 --repo github.com/O/R | confirmed: behind_by 0 |
| W24 | pullRequests.runAction update-branch (rebase) | decoded (7341 ms) | graphql viewerCanUpdateBranch; graphql viewerDidAuthor; pr update-branch 140 --rebase | confirmed: behind_by 0 |
| W25 | pullRequests.runAction enable-auto-merge | decoded (2837 ms) | graphql viewerDidAuthor; pr merge 141 --auto --squash | confirmed: autoMergeRequest {"mergeMethod":"SQUASH"} |
| W26 | pullRequests.runAction disable-auto-merge | decoded (2180 ms) | graphql viewerDidAuthor; pr merge 141 --disable-auto | confirmed: autoMergeRequest null |
| W27 | pullRequests.runAction revert | decoded (3542 ms) | graphql viewerDidAuthor; graphql pullRequest(number; graphql (stdin) | confirmed: #142 "Revert "Add check note 4"" opened (closed again by the probe) |
| W28 | pullRequests.runAction approve-workflows (cross-repository) | decoded (2493 ms) | graphql viewerDidAuthor; graphql viewerCanUpdateBranch; run list --commit 4bdd36b4b54b3a716bccaf945e80f3c1f78351cd --branch do; pr list --state open --head docs/contributing --limit 1001 --json numb | confirmed: nothing to approve: 0 runs await approval (the playground has no workflow, and GitHub asks approval only for outside or first-time contributors; the second account is a collaborator) |
| W29 | pullRequests.submitReview (request-changes, 2 line comments) | decoded (2080 ms) | graphql viewerDidAuthor; api POST repos/O/R/pulls/143/reviews | confirmed: review CHANGES_REQUESTED, 2 line comments at 3,7 |
| W30 | pullRequests.submitReview (approve) (run 1; run 3: transient "PullRequestOperationError: GitHub CLI command failed.") | decoded (1500 ms) | graphql viewerDidAuthor; api POST repos/O/R/pulls/131/reviews | confirmed: latest review APPROVED |
| W31 | pullRequests.runAction update-branch (stack rebase) | decoded (8985 ms) | api repos/O/R/stacks?pull_request=134; api repos/O/R/stacks/135; graphql viewerCanUpdateBranch; graphql viewerDidAuthor; api graphql owner=daehyeonmun2021 name=playground; api graphql id=PR_kwDOU_luzM8AAAABHJGuVg sha=a551f7dedf17f668d06630f5d | confirmed: bottom behind main 4 → 0; top still on chore/check-10071440-1; heads b5ae551, be4e6a8 |
| W32 | pullRequests.runAction merge (whole stack, merge-async) (run 1; run 3: transient "PullRequestOperationError: GitHub CLI command failed.") | decoded (12033 ms) | api repos/O/R/stacks?pull_request=122; api repos/O/R/stacks/123; graphql viewerDidAuthor; api PUT repos/O/R/pulls/122/merge-async merge_method=squash; api repos/O/R/pulls/122/merge-async/89e008c2-06d4-4cfc-9194-0c4c94f1ff; graphql stackEntry | confirmed: #121 merged true, #122 merged true |

**Live drive** (agent mode, 1280×840, the branch's development build paired with the primary lane
server; GitHub effects read back with the lane gh; threads linked to seeded pull requests by RPC):

- Attempt 1 (base app code: no app source differs between `38352ceaf` and the drive's build):
  the Changelog thread's row showed `#106: Add a changelog` and never split (no state, checks or
  action) in 20 s. Cause found offline: the let-go read above. The server answered the same
  `pullRequests.detail`/`checks` calls in 0.8 s when replayed.
- Attempt 2 (with the fix): `#106` split (Open, `main ← feature/changelog`, All checks passed, 1
  file, +3 −0, Merge); its hover card showed; Merge opened the sheet ("This merges #106 using
  merge."), the confirm merged it on GitHub (merged by `daehyeonmun2021`) and toasted "Pull request
  merged". `#107` Draft → Ready: GitHub `draft: false`, "Marked ready for review". `#111` failing:
  the checks popover ("Some checks were not successful", "1 of 2 failing", `ci/test` 2 tests failed,
  Details, Show all); Fix created the worktree `fix/text-helpers` in the lane T3 home, toasted
  "Checkout ready — The task is in the composer", and the composer showed the pull request chip
  `#111`. The server's trace shows `ProjectSetupScriptRunner.runForThread` inside the clone's
  `preparePullRequestThread`, so the clone passed the draft's thread id. `#110` (conflict) offered
  Merge, not Resolve: GitHub had just reported its mergeability as unknown after `#106` merged,
  and the reference's `resolveThreadPanelPullRequestAction` offers Merge for that too. The drive
  opened that Merge sheet (not confirmed) and stopped there (the sheet made the next control inert).
- After the drive limit, by RPC with the clone's own payloads (`target/…/drive/rpc-checks.mjs`, not
  committed): with the setup script in the project's settings override (this server reads project
  scripts from settings; the drive's prep had put it on the project record, so no script ran), a
  worktree hand-off of `#110` ran the setup script once for the draft's thread; R3:
  `git.resolvePullRequest #112` answered "Add a banner helper" (`feature/banner`), and
  `git.preparePullRequestThread` local mode switched the project checkout `main → feature/banner`
  through `gh pr checkout` (switched back after).

Evidence (before/after, the only UI change):
![row](https://raw.githubusercontent.com/ccheever/exact2/6aa7178122ca1ac95280c951ff3d81a4b6037dd1/real-github-lane/01-pr-row-real-github-before-after.png)

**Not verified, with blockers:**

| Item | Done instead | Blocker |
| --- | --- | --- |
| The Pull Requests page with real data, its row-number and detail-header menus (context-menu-gaps) | probe R3–R7 decode the same list; the menus' items come from `linkMenu` (provider + URL), unit-tested in `context-menus.test.ts` | the one-drive limit (user rule 3) was spent: attempt 1 found the let-go bug, attempt 2 stopped at the conflict row. Native menus also need a normal launch with real input: under the agent `T3ContextMenu` answers "dismissed" without popping up, and the screen was locked (`CGSSessionScreenIsLocked`) |
| R3 checkout dialog UI (select-on-open, Resolving debounce, Check out) | its two server calls on real GitHub by RPC (above) | the one-drive limit (user rule 3) |
| Composer chip hover and press | the chip showed `#111` after Fix | the one-drive limit (user rule 3) |
| Hand-off: quit, relaunch and send the draft into the setup terminal's thread | the setup script runs once for the draft's thread (RPC) | sending needs a signed-in provider (the lane's Codex is unauthenticated; `provider-sign-in-and-install`) |
| Workflow approval with a run to approve | the RPC decodes; nothing awaits approval | GitHub asks approval only for outside or first-time contributors' fork runs; the only other account is a collaborator |

## Progress

2026-10-07: lane, seed, probe, unit fallbacks; the user signed both accounts in; the playground
created and seeded; three probe runs; one drive and one retry; the let-go fix; records moved off the
fake `gh`. The repository, its fork and both lane logins stay in place for the later pull request
tasks.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (pre-login) | `cee18d42d` | `lane.test.ts` 11/0, `pr-profiles-injection.test.ts` 11/0; lane server smoke without a login (wrapper refused, nothing sent) | this record | the user's login |
| 2 (after login) | branch tip | seed ×5, probe ×3, drive ×2, RPC re-checks; checks in the PR body | Results above, before/after image | the rows in "Not verified" |

## Next action

Review. A later pull request task (or an attended session) drives the Pull Requests page and the
native number menus on a normal launch, and the checkout dialog, against the playground. The
repository, its fork and both lane logins stay in place.
