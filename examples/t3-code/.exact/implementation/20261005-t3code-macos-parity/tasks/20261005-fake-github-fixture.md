---
name: 20261005-fake-github-fixture
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Fake GitHub CLI fixture for pull request reads and writes

## Outcome

A lane can start its isolated fixture T3 server with a fake `gh` first on `PATH`, and every
`gh` command that the pinned reference server issues for the pull request features in
`20261005-pr-conversation-and-refresh`, `20261005-pr-header-actions-and-stacks`,
`20261005-pr-handoffs-and-quick-actions`, `20261005-pr-writing-and-metadata`,
`20261005-pr-code-tab` and `20261005-pr-links-previews-and-routing` gets a stateful, logged answer. Reads: detail, activity (comments, reviews, line
threads, commits, dismissals), reviewer and label candidates, base comparison, permissions,
list search with cursors, stats, stack membership and stacks, files and patches, file
contents, viewed marks. Writes: close, reopen, ready/draft, merge (also auto-merge),
update-branch, comment, review, edit of PR and comments, reply, resolve, reactions,
reviewers, labels, viewed marks, revert, workflow approval, stack merge and rebase.
Named viewer profiles make permission states reachable; error and delay injection make error
and loading states reachable; `calls.ndjson` records arguments **and stdin bodies**.

**This is verification apparatus and needs the user's approval before `prepare`**
(CLAUDE.md: agents add no apparatus without a human saying so; it is the plan's fake-`gh`
write-verb item, earlier named `20261005-fake-github-writes`). It is not part of the app's
build or runtime.

## Scope and exclusions

Included: the verb table in Implementation notes; the state model; profiles; injection;
logging; lane isolation (per-lane state and log paths); a self-test; a small RPC probe that
sends the PR RPCs to the lane server; `AGENT-HANDOFF.md` "Fake gh" section.
Excluded: other hosts' CLIs (`glab`, `az`, `fj`/`tea`, Bitbucket API; plan decision U21); any
network access; live GitHub; changes to the reference server;
app code.

## Context and guidance

Parent specification: [spec](../spec.md) ("GitHub features go through the server's `gh`;
verified against a fake `gh` now, live with disposable accounts later"). Source behavior: the
existing fake in mc-orch `target/t3-ui-parity/lanes/r6-pr/fakegh/` (`gh.mjs` 144 lines,
`bin/gh`, `state.json`, `state.seed.json`, `calls.ndjson`), described in `examples/t3-code/AGENT-HANDOFF.md` ("lanes/r6-pr"). The reference
server's calls are in T3 Code `1e2ecbd975` `apps/server/src/pullRequest/GitHubPullRequestCli.ts`,
`githubStackActions.ts`, `gitHubPullRequestJson.ts` (GraphQL documents), `GitHubPullRequestProvider.ts`
(`gitHubViewerPermissions`).
Library revision: `20261005-platforms-v3`. Selected topics: testing-and-debugging (record
source/build identity; an exit code of zero is not proof; keep failed evidence), platforms
(label host and mode). The fake runs outside the exact2 app, so the other topics do not apply;
anything about lane start-up is unknown in the library and rests on the clone's own lane tools
(copied by `20261005-clone-on-exact2-main` into this worktree's `target/`).
Consumer framework revision and toolchain: the pin chosen by `20261005-clone-on-exact2-main`;
pinned Bun 1.4.2 (`~/.bun-1.4.2/bin`).
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find
it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
The fake lives under `examples/t3-code/tools/fakegh/` (decision U23, 2026-10-05); other
source-control CLIs are U21.

What the current fake does (mc-orch `gh.mjs`): serves auth, `api user`, the core detail query
(matched by `viewerCanUpdateBranch`, :66-79) and the viewer query (`viewerDidAuthor`, :93-97),
`pr view/list/checks`, `repo view`, `pr ready`, `pr merge --merge|--squash|--rebase`; returns `[]`
for `…/pulls/N/files` (:136) and `…/stacks` (:138); refuses every GraphQL mutation (:86) and
`--auto` (:124); fails other GraphQL as "unserved" (:99). It hard-codes the viewer as author and
ADMIN of every PR (:60-66, so Approve and Request changes can never be offered), reads stdin
only for `--input -` (:16), logs no bodies (:17), writes `calls.ndjson` beside the script (shared
by lanes), and `bin/gh` execs `/opt/homebrew/bin/node`. Pull Requests list data is marked
live-credentials-only in `AGENT-HANDOFF.md` (the search query is not served).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Lane tools and `runtime-f870c41` are in this worktree's `target/` | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | `ref-build` has produced `target/t3-ref/runtime-1e2ecbd975`; this ticket runs after it, because `runtime-f870c41` predates `f4f3abf71e` (paginated review replies; touches `GitHubPullRequestCli.ts`, `GitHubPullRequestProvider.ts`, `PullRequestService.ts`) and the verbs must be proven at the pin | pending |
| recorded decision | Apparatus approval for the fake `gh` extension and the RPC probe (plan decision U2) | none | User approves | pending |
| recorded decision | Location of the fake `gh` (U23, decided: `examples/t3-code/tools/fakegh/`) and other source-control CLIs (U21, open) | none | U21 answered at `prepare` | U23: user 2026-10-05 |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 over the reference server source and the
mc-orch fake; plan issue drafts not applicable (the fake runs outside exact2). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| no framework blocker found | Fake `gh` outside the app | Runs under Bun on macOS | none | Proceed after approval |

## Implementation notes

The fake-`gh` lane backend starts like every lane server, with
`T3CODE_TELEMETRY_ENABLED=false` (issue X39), so no test event leaves the machine.

Verb table (server operation → `gh` call → fixture effect). Reference lines: `GitHubPullRequestCli.ts` (`CLI`) and `gitHubPullRequestJson.ts` (`JSON`).

| Used by | Server call | Fixture answer or state change |
| --- | --- | --- |
| `20261005-pr-conversation-and-refresh` | `pr view N --repo H/R --json author,comments,reviews,commits` (CLI:2197) | comments, reviews, commits from state |
| `20261005-pr-conversation-and-refresh` | GraphQL `reviewThreads(first:100` + `viewer{login}` (JSON:871), `REVIEW_DISMISSED_EVENT` (:1137), `nodes(ids:` avatars (:675), `node(id: $threadId)` (:937) | threads with comments, reactions, resolved/outdated; dismissal messages |
| `20261005-pr-header-actions-and-stacks`, `20261005-pr-writing-and-metadata` | `viewerCanUpdateBranch` + `baseRef{compare…behindBy}` (JSON:2457); viewer query (:2763) | per-profile permission, `behindBy`, `viewerDidAuthor` from PR author |
| `20261005-pr-header-actions-and-stacks` | `pr close\|reopen N`; `pr ready N [--undo]`; `pr merge N --<m>`; `--auto --<m>`; `--disable-auto`; `pr update-branch N [--rebase]` (CLI:1058) | state, `autoMergeRequest`, `behindBy` reset |
| `20261005-pr-header-actions-and-stacks` | node-id query (JSON:980) then `mutation revertPullRequest` (:1073) via `--input -`; run list read then `POST …/actions/runs/ID/approve --silent` (CLI:2636) | new revert PR; runs approved |
| `20261005-pr-header-actions-and-stacks`, `20261005-pr-handoffs-and-quick-actions` | `GET …/stacks?pull_request=N`, `…/stacks/NUM`; `PUT …/pulls/N/merge-async -f merge_method= -f merge_action=default -f sha=`; poll `…/merge-async/UUID`; rebase: `-f query=…headRepository{viewerPermission} maintainerCanModify…` and `mutation…updatePullRequestBranch(…REBASE)` (`githubStackActions.ts:195-418`) | stack fixtures; pending→done job; head SHA moves |
| `20261005-pr-handoffs-and-quick-actions`, `20261005-pr-links-previews-and-routing` | search `query($q…){search(query:$q,type:ISSUE,…)` via stdin (JSON:817); stats aliases `s0: repository(…)` (:1781); `stack{…}stackEntry{…}` aliases (:1799) | >99 PRs, `updated:<=` cursor, `sort:updated-desc`, memberships |
| `20261005-pr-writing-and-metadata` | `pr comment N --repo … --body-file -` (CLI:2664, stdin) | comment appended (id, author, body) |
| `20261005-pr-writing-and-metadata`, `20261005-pr-code-tab` | `POST …/pulls/N/reviews --input -` (CLI:2682) | review + line comments with `position` |
| `20261005-pr-writing-and-metadata` | mutations `updatePullRequest` (JSON:1066), `updateIssueComment` (:1083), `updatePullRequestReviewComment` (:1087), `addReaction`/`removeReaction` (:1045,1049), each after `repository.pullRequest{id}` (:980) or `node(id: $subjectId)` scope (:1006) | title/body/comment text, reactions with actors |
| `20261005-pr-writing-and-metadata` | `assignableUsers` (JSON:2515); `labels(first… orderBy` (:2646); `POST\|DELETE …/pulls/N/requested_reviewers --input -`; `POST …/issues/N/labels --input -`, `DELETE …/labels/<name>` (CLI:2472,2513) | candidates; requests; labels |
| `20261005-pr-code-tab` | `pr diff N --color never`; `…/pulls/N/files?per_page=&page=`; `…/commits/SHA?… --jq '.files // []'`; `…/pulls/N --jq '[.base.sha, .head.sha]\|@tsv'`; `…/contents/<path>?ref= --header 'Accept: application/vnd.github.raw+json'` (CLI:1393-1550) | patch, file entries, base/head contents |
| `20261005-pr-code-tab` | `files(first:100…){path viewerViewedState}` (JSON:2877); `markFileAsViewed\|unmarkFileAsViewed` aliases `f0…fN` (:2962); `addPullRequestReviewThreadReply` (:969); `resolveReviewThread\|unresolveReviewThread` (:1053,1057) | viewed marks, replies, resolution |

State model: per PR `author`, `viewerDidAuthor` (derived), `isCrossRepository`/`maintainerCanModify`,
labels, review requests, `autoMergeRequest{enabledAt,mergeMethod}`, `behindBy`, comments,
reviews, threads, commits, files with patches and base/head contents (matching the bare repo),
viewed marks, workflow runs, stack id and layers; repo labels catalog and assignable users;
global `viewerPermission`. Generated cases: a PR with 130 files (`pr diff` answers GitHub's 406
so the files API pages 100 + 30), a repository with 130 PRs (search cursors by `updated:<=`),
two lanes sharing one account id with separate state and log files (routing).
Profiles (`FAKE_GH_PROFILE`): `admin-author` (today), `admin-reviewer`
(not author), `writer`, `triage`, `reader`, `contributor-author` (READ, author). Injection:
`failNext{match,message,code}` and `delay{match,ms}` in state or via `gh.mjs --inject`. Log path
and state path come from `FAKE_GH_LOG`/`FAKE_GH_STATE` (per lane). Run under Bun; `bin/gh`
no longer names Homebrew Node.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Every verb row behaves | `state.seed.json` reset | `bun test <fakegh dir>` (argv + stdin in, stdout + state diff out, one test per table row) | All pass; seeded PRs 101–107 behave as before (`AGENT-HANDOFF.md` lanes/r6-pr) | macOS, Bun | test log |
| Pinned server accepts the answers | Lane server on a 16xxx port, isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME; fake first on `PATH` | RPC probe calls each PR RPC in the table | Each decodes; no `calls.ndjson` handler ends in `unserved`/`refused` for a table row | macOS | probe JSON + `calls.ndjson` |
| Writes change state and log bodies | Same | Probe: comment, review with 2 line comments, title edit, reaction, reviewer, label, close, reopen, auto-merge, update-branch | `state.json` diff matches; each stdin body present in the log; no token or pairing code in either | macOS | state diffs, grep log |
| Profiles | One server run per profile | Probe `pullRequests.detail` | `viewerPermissions` equals the expected set per profile (reader: no actions; triage: labels only; contributor-author: close/reopen/ready/draft; admin-author: verdict `comment` only; admin-reviewer: all verdicts) | macOS | probe JSON |
| Injection | `failNext` on activity; `delay` 2000 ms | Probe | `PullRequestOperationError` once, then success; delayed answer arrives ≥2 s | macOS | probe JSON |
| Lane isolation | Two lanes at once | Run both | Separate state and log files; `lsof` shows only 16xxx ports; real `~/.t3`, `~/.config/gh` untouched; no network socket from the fake | macOS | `lsof`, mtime listing |
| Same at the pin | `target/t3-ref/runtime-1e2ecbd975` from `ref-build` (`20261005-desktop-oracle-and-trace`) | Repeat the probe | Same results, or each difference recorded and fixed | macOS | probe JSON |
| Docs and gates | `git add -A` | `bun scripts/caps.mjs`; the five repository checks | Pass; `AGENT-HANDOFF.md` lists verbs, profiles, env vars | macOS | logs |

Task-owned source paths: the fake `gh` directory (location per the commit-or-ignore decision),
its self-test, the RPC probe, `examples/t3-code/AGENT-HANDOFF.md`.
Required environment: Bun 1.4.2, the pinned reference runtime copy, no network.

## Progress

2026-10-07: replaced by real GitHub (user decision); see [20261007-real-github-lane](20261007-real-github-lane.md).

2026-10-06: on hold (user decision). Tasks that need a sign-in (GitHub, provider accounts, T3 Connect) do not start until the user lifts the hold.

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

None: superseded by [20261007-real-github-lane](20261007-real-github-lane.md). Its verb table remains the
coverage list the real lane's probe proves.
