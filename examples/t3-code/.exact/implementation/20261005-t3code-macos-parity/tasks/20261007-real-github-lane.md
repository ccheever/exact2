---
name: 20261007-real-github-lane
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-real-github-lane
pr_url: null
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
tasks' features; GitHub stacks and workflow approval where the account cannot produce them (each
row names its reason); any repository other than the sandbox and the second account's fork.

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
| recorded decision | Sandbox owner, name, visibility | none | User approves at the login stop | pending |
| user action | The user signs both accounts in to the lane config dirs | none | `bun lane.mjs whoami` names both | pending |
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

**Seed** (`seed.mjs`). Base history of seven commits with fixed contents, author and dates (the same
SHAs in every checkout); labels; the second account as collaborator (invitation sent by the
primary, accepted through the second account's lane gh) and its fork; pull requests: `open-clean`
(success status, labels, review requested from the second account), `draft`, `closed`, `merged`,
`conflict` (branched before main changed the same line), `failing` (a failure status), `running`
(pending), `behind` (branched three commits back), `many-files` (310 files, so GitHub answers 406
to `pr diff` and the server pages the files API 100 at a time), `second-review` (authored by the
second account; the primary's dismissed request-changes review with four line threads: one
unresolved, one resolved, one outdated by a follow-up commit, one of 12 comments to page; a
standing request-changes review; comments from both; reactions), `primary-review` (approved and
commented by the second account), `cross-repo` (from the second account's fork), and 105 open bulk
pull requests (the list pages past its 99-row slice). Idempotent by branch name and invisible body
markers; a drifted pull request is put back in one call or replaced by the next generation.

**Probe** (`probe.mjs`). Per RPC: the decoded summary or the typed failure, the `gh` calls the
server made for it (from the wrapper log), and for writes a read-back from GitHub with the lane
gh. Writes go to the probe's own pull requests (`probe/<run>/…`, label `probe`). Profiles: one
server per account, `pullRequests.detail` as admin author, admin reviewer, write reviewer, write
author and cross-repository author.

**Unit fallback** (`pr-profiles-injection.test.ts`, 11 tests). Profiles two accounts cannot hold
on a personal repository (read, triage, a read-only fork author, maintain) through the server's
`gitHubViewerPermissions` into the clone's `presentDetail` and `rowAction`; injected failures (detail
failure and not-found, activity failure, refused write, candidate failure) and delays (the panel's
loading state until the answer lands; the row's 30 s retry). `lane.test.ts` (11 tests) covers the
wrapper's refusal and pass-through, the server environment, the login shell's PATH, the seed's
deterministic history and state rules, and the probe's record.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| The server's `gh` is the lane's | Lane server, shared login | `bun lane.mjs which primary`; probe `S-*` rows and `ghConfigs` | the wrapper and the lane config dirs only; no call refused | macOS | probe JSON |
| Seed | Approved sandbox | `bun seed.mjs` twice | the second run creates nothing | macOS | seed output |
| Every read decodes | Seeded sandbox | `bun probe.mjs` | every `R*`, `D-*`, `P*` row decoded | macOS | probe table |
| Every write has its effect on GitHub | Same | `bun probe.mjs` | every `W*` row confirmed by read-back, or not producible with its reason | macOS | probe table |
| States GitHub cannot produce | — | `bun test examples/t3-code/pr-profiles-injection.test.ts` | pass | — | test log |
| Built pull request behaviors, live | Lane server on the sandbox, lane app copy | one macOS drive | each behaves as the reference; fixes with before/after pairs | macOS | drive record |
| Records | — | — | fake-github-fixture blocked and superseded; six PR tasks depend on this task; plan, spec, STATUS, AGENT-HANDOFF updated | — | diff |
| Gates | `git add -A` | clone checks, `bun scripts/caps.mjs`, the five checks | pass | macOS | numbers below |

## Progress

2026-10-07: lane recipe, seed, probe and unit fallbacks written and tested offline; records moved
off the fake `gh`. Waiting for the user's sign-in and sandbox approval.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (pre-login) | branch tip | `lane.test.ts` 11/0, `pr-profiles-injection.test.ts` 11/0; lane server smoke without a login (wrapper refused, nothing sent) | this record | the user's login |

## Next action

After the login: check both logins with the lane gh, create the sandbox as approved, seed, probe,
one live drive, then finish the records and the PR. The sandbox, its fork and both lane logins
stay in place for the later pull request tasks.
