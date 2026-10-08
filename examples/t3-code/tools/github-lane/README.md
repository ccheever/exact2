# Real-GitHub lane

The clone's pull request features are verified against real GitHub (user decision 2026-10-07,
which replaced the fake `gh` of `20261005-fake-github-fixture`). A lane T3 server runs on
isolated homes, and the `gh` it spawns is the real GitHub CLI on a lane-only config dir that
the user signed in to. Task record: `.exact/implementation/20261005-t3code-macos-parity/tasks/20261007-real-github-lane.md`.

Nothing here reads, prints or commits a token. The lane never uses `~/.config/gh`, the
keyring, `gh auth token` from a normal shell, `~/.t3`, port 3773 or the `t3code` URL scheme,
and touches no repository but the approved sandbox and the second account's fork of it.

## Directories and variables

| Variable | Default | Holds |
| --- | --- | --- |
| `T3_GITHUB_LANE_SHARED` | `<checkout>/target/t3-ui-parity/github-lane` | `gh/` (primary account), `gh-second/` (second account), `sandbox.json` (the sandbox, the logins, the seeded numbers). Point every worktree at the base checkout's copy so the login is reused. |
| `T3_GITHUB_LANE` | `<checkout>/target/github-lane` | this checkout's `bin/<account>/gh` wrappers, `home/<account>` (`.zprofile`, `.gitconfig`), `servers/<account>` (T3 home, Codex, Claude, XDG, tmp, the sandbox clone, `server.log`, `server.pid`), `work/` (the seed's object store), `logs/gh-calls.tsv`, `logs/probe-*.json` |
| `T3_LANE_SERVER` | the staged release (`bun examples/t3-code/stage-runtime.mjs`), unpacked into `runtime/` | another server: a `t3` executable or a `bin.mjs` (run by `node`) |
| `T3_GITHUB_LANE_GH` | `gh` on `PATH` | the real GitHub CLI the wrappers exec |

A lane server's environment is built from nothing (`serverEnv`): `HOME`, `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_HOME`, `TMPDIR` under its server dir,
`T3CODE_TELEMETRY_ENABLED=false`, `GH_CONFIG_DIR` set to the account's lane config dir,
`GIT_CONFIG_NOSYSTEM=1`, ports 16520 (primary) and 16521 (second) by default, `--port` within 16500-16799 (a task lane's own hundred). The
server rebuilds `PATH` from `zsh -ilc`, so the isolated home's `.zprofile` puts the lane's
`bin/<account>` first; `bun lane.mjs which <account>` shows what that shell resolves. The
wrapper logs every call (argv only) to `logs/gh-calls.tsv` and refuses to run unless the config
dir holds its own token, so a missing login cannot fall through to the keyring. Git in the
isolated home uses no system or keychain helper, only `gh auth git-credential` of the lane gh.

## Sign-in (the user, once)

Run in a terminal, with the shared dir of the base checkout. `--insecure-storage` keeps the
token in that dir's `hosts.yml` instead of the macOS keychain, where it would overwrite the
machine's own `gh` login.

```sh
GH_CONFIG_DIR=<shared>/gh        gh auth login --hostname github.com --git-protocol https --web --clipboard --insecure-storage --scopes repo,read:org,workflow
GH_CONFIG_DIR=<shared>/gh-second gh auth login --hostname github.com --git-protocol https --web --clipboard --insecure-storage --scopes repo,read:org,workflow
```

The first is the primary account (it owns the repository), the second its collaborator and
fork owner. Sign the browser in to the matching account before entering the code at
github.com/login/device. Signed in on 2026-10-07: primary `daehyeonmun2021`, second
`daehyeon-mun`.

## The repository (user decision 2026-10-07)

`daehyeonmun2021/playground`, public, with the second account's fork `daehyeon-mun/playground`.
It is a plain playground: no code from this repository or T3 Code, and no "t3", "T3 Code",
"sandbox", "exact" or clone wording in its name, description, README, branches, pull request
titles and bodies, labels or commit messages (`lane.test.ts` checks the seed's). Branches are
`feature/…`, `fix/…`, `docs/…`, `chore/note-NNN`; the probe's are `chore/check-<run>-<n>`.
main requires the `ci/build` status (admins included), so merges wait for it and auto-merge can
be armed. Checks are commit statuses (`ci/build`, `ci/test`); there is no workflow. The seed
refuses a repository it did not create (`sandbox.json` keeps the id). The `daehyeon-mun` token
can reach other organizations: use it for nothing but this repository and its fork.

## Use

```sh
export T3_GITHUB_LANE_SHARED=<base checkout>/target/t3-ui-parity/github-lane
bun lane.mjs setup && bun lane.mjs whoami
bun seed.mjs                       # idempotent; --repo owner/name --create --visibility public made it once
bun probe.mjs                      # starts both servers, probes every RPC, stops them
bun lane.mjs start primary && bun lane.mjs project primary && bun lane.mjs pair primary   # for a live drive
bun lane.mjs stop primary
```

The seed finds its pull requests by branch (`feature/changelog`, then `feature/changelog-2`) and
its comments by an invisible `<!-- ref:… -->` marker, puts a drifted one back where one call does
it (draft again, closed again, reopened, merged), and otherwise opens the next generation, so a
drive that merged or readied a seeded pull request is undone by the next `bun seed.mjs`. It also
keeps a two-layer GitHub stack (`POST /repos/{o}/{r}/stacks`). The probe writes only to its own
pull requests and stack. A worktree setup script for hand-off checks goes into the project's
settings override (`ensureProject(…, { scripts })`): this server reads project scripts from
settings, not from the project record. The repository, its fork and both logins stay in place
for the later pull request tasks.
