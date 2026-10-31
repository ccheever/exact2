# Second environment

Extra T3 environments for drives of the clone: lane T3 servers (the official runtime the app
stages, `server-runtime/`), each on its own homes under `target/`, seeded with projects and
threads through the server's own API, and paired into the app as saved environments. With one
paired the app has a background environment (the sidebar's background rows and drops,
Settings › Projects for a project only a background environment has, its Mark unread); with
two, and "This machine", Settings › Connections has a machines list (`../machines/`).

## What it isolates

A lane server's environment is built from nothing (`serverEnv`): `HOME`, `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_HOME`, `TMPDIR` under its own directory, telemetry off,
none of the calling shell's tokens. Every built-in provider's binary is pinned to
`/nonexistent/t3-fixture/<id>` in its settings. Its HTTP(S) goes to a proxy on a closed loopback
port (`T3_FIXTURE_OFFLINE_PORT`, default 16599; Node's fetch under `NODE_USE_ENV_PROXY=1`, Git and
curl by the variables), so it reaches no provider, no registry and no GitHub; a start refuses
while anything listens there. A sandbox profile cannot do this: the server runs the setuid
`/bin/ps`, which a sandboxed process may not. A shim first on its `PATH` answers the server's
`scutil --get ComputerName` with the lane's label, so environments read "Lane Second" and
"Lane Third", not this Mac's name.

Ports are 16500-16599 (`T3_FIXTURE_PORTS=lo-hi` names another lane's range; 3773 never):
"This machine" 16500, `second` 16510, `third` 16511, the usage hub 16590. Everything is written under
`T3_FIXTURE_ROOT` (default `<checkout>/target/t3-fixtures`): `runtime/` (the unpacked release),
`envs/<name>/` (homes, `repos/`, `server.log`, `server.pid`, `environment.json`), `local/` (This
machine's home for `--local` drives), `drives/`.

## Use

Stage the runtime and build the bundle first (the app README's Build and run). From
`examples/t3-code/tools/second-environment`:

```sh
bun env.mjs start second [--runtime <version>] [--scratch] && bun env.mjs seed second [--turns] [--more-turns] [--limits] [--pull-request]
bun env.mjs status                       # pid, port, label; projects and threads with their ids
bun env.mjs drive --local --pair second --storage s1 -- "tree sidebar" "screenshot /abs/out.png window"
bun env.mjs stop all
```

The seed: `shared-notes` in both environments (one Git origin, so the clone groups it as one
logical project) with a thread each, and in `second` only `background-only` with "Background
thread (no run)" and "Background follow-up" and the action "Echo test" (its project settings'
action to edit and delete), and `empty-project` with no thread. Threads are created with
`thread.create` and never get a turn.

`seed second --turns` makes the ACP stand-in (`../acp-standin/`) a provider of that server and
adds threads with a turn on it, each with a context meter (the stand-in reports its usage); their
composer's `$` menu offers the stand-in's skill, Frontend Design:

- "Background thread (completed turn)" in `background-only`: one completed turn, visited at its
  completion, so it starts read.
- "Turn with file edits" in `agent-edits`: the turn edits `README.md` and writes `notes/plan.md`
  and `notes/outline.ts`, so the diff panel's Turn submenu has "Turn 1" and the turn's diff has
  the three files (collapsed: a drive presses `diff-toggle-README.md` before its rows exist), and
  the Files preview of `notes/outline.ts` has a gutter. The edits stay in the working tree, so
  `agent-edits`' Uncommitted scope shows them too.
- "Edit awaiting approval" in `agent-edits`, in the runtime mode "approval-required": the turn
  waits on the stand-in's permission request (Allow once, Always allow, Reject; what the card
  does not show as buttons is in its `approval-more-<id>` menu). The request lives in the
  stand-in's process, so a server restart drops it; seed again for a new one.

`seed second --more-turns` adds to `agent-edits`, besides the `--turns` threads: "Second edit awaiting approval" (a
second approval, so one can be approved and one declined), "Italic answer" (emphasis in prose and a list, for a font's
italic face) and "Turn held open", whose turn the stand-in holds running until `bun ../acp-standin/install.mjs release
--home <its t3home>` or a steer or Stop cancels it (for a send while a turn runs). The stand-in's sessions offer a
reasoning effort select (Low, Medium, High, Max), so its composer shows effort as a word or bars, and "Which effort?"
replies with the one the turn ran at.

Run again, the seed skips a thread that has its turn (or its pending approval) and gives one to a
thread an interrupted seed left without.

`seed second --limits` starts the usage hub stand-in (`usage-hub.mjs`, port 16590,
`T3_FIXTURE_HUB_PORT`) and adds it to that server as the usage limit source "Lane hub", as
Settings › Providers' Add hub does; the seed waits until the server has read its accounts. The
hub answers the CLIProxyAPI management calls the server makes (reference
`apps/server/src/usage/cliproxyApi.ts`) for three Codex accounts and two Claude ones, so Usage ›
Limits draws a Codex pool and a Claude pool with a segment per account (`usage-seg-<pool>-<window>-<n>`,
indexes from 0).
Two Codex accounts bank reset credits, so their popover has "Use reset" and its confirm; a
consumed credit empties that account's session window. The hub's resets are real times from now, so
drive Limits with `--epoch now`; at the driver's default epoch (2026-01-01) every window reads as resetting months
away and its even-pace mark sits at the bar's end. `stop all` stops the hub and the GitHub API stand-in too.

`seed second --pull-request` puts a `gh` stand-in (`gh-standin.mjs`) first on that server's
`PATH` and adds "Notes branch thread" to `shared-notes` on the branch `feature/notes`; the seed
waits until the server has found the stand-in's open pull request, #7 "Keep the notes in one
place", for the branch, then links it to the thread (the panel's Linked pull requests list it,
with Shift's quick actions). The pinned server reads pull requests from GitHub's API over HTTPS
with the token `gh auth token` gives it, so the seed starts it again behind the GitHub API
stand-in (`github-api-standin.mjs`, port `T3_FIXTURE_GITHUB_PORT`, default 16592, and the next
port): a proxy that tunnels `api.github.com` to its own HTTPS server, whose certificate a CA made
under `<root>/github-api` signs (the server trusts it by `NODE_EXTRA_CA_CERTS`), and refuses every
other host. It answers the head lookup, the lookup by number, the summary, the Pull Requests page's
listing (the search, a repository's list and the counts read after them)
(`api-calls.log` there lists each request), and the detail panel (`github-api-detail.mjs`): the
detail with four checks (passed, skipped, running, and a status without a link), an issue comment
and a review, a review thread on `README.md` line 3 (its first comment links `notes/plan.md`) with a reply, the files viewed, each file's line
counts (the change breakdown: code, and docs for the Markdown), and a patch
long enough to scroll (`GET pulls/7` as a diff). The pull request comes from the viewer's own fork, so the
0.1.5 server offers Allow edits from maintainers (`maintainerCanModify`, `UpdatePullRequest`), and the activity names its
commit by node id with its stats (`PullRequestCommitStats`). Comment, Close (and Reopen), Viewed (`SetFileViewed`, one
alias per file) and Allow edits from maintainers change what the next read answers until the stand-in stops; Merge,
reactions, title and description edits and the rest are not answered.
The
stand-in answers the calls the reference server makes for these (`GitHubCli.ts`,
`GitHubPullRequestCli.ts`; `gh auth token` gets a placeholder, which the server hands back only to
the stand-in), logs each to `envs/<name>/gh-calls.log` and refuses every other one.

`drive` runs `scripts/agent.mjs macos` on the newest bundle (`EXACT_MAC_BIN` overrides) at
1280x840 (`--size`, `--storage`, `--epoch` and `--seed` pass through to the driver; a drive against a server an earlier drive used needs a new `--seed`, since two launches with one seed mint the same command ids). In a fresh store the app opens its welcome wizard: the drive pairs each `--pair`
environment there (a fresh single-use token each; the driver's echo of it is redacted), then
finishes the wizard with "Do not import projects", and runs your ops. `--local` also starts
"This machine" on the fixture's local home and port, with that home's provider directories and
the offline proxy, so its wizard offers no history of this Mac. `--keep-welcome` stops before
finishing the wizard. A drive waits 5 s for the wizard, 20 s while the local home has not unpacked
the pinned runtime yet (its first start). Agent runs keep pairings and credentials in memory, so every drive pairs
again and nothing reaches the Keychain. A background row's thread is
`thread-fleet:<environment id>:<thread id>` (ids from `status`).

`start --runtime <version>` runs another release than the pin: `examples/t3-code/.runtime-cache/t3-<version>-darwin-arm64.tar.gz`,
which stage-runtime keeps (`--runtime 0.1.4-nightly.20261027.3770`, a server older than the client). A plain older
server has no self-update, so the update dialog lists it under "needs a manual update"; for one it offers to update, put
the older-server stand-in in front (`../machines/`, `older`). `start --scratch` lets the server offer a folder for threads
without a project: the fixture root is inside this checkout, a Git work tree, and the server refuses one there
(reference `ManagedProjectFolders.ts`), so a lane offers none unless Git's search stops at its directory. `no-scratch
<t3 home>` does the opposite for This machine's home (`T3_LOCAL_HOME`, outside the checkout): its server then offers no
folder, and the palette's New thread without a project goes to a machine that does. `samples <dir>` writes `sample.py`,
a one-page `doc.pdf` and `clip.mp4` (from `apps/video-player`) to attach as chips.

Another lane's ports: `T3_FIXTURE_PORTS=16300-16399 T3_FIXTURE_OFFLINE_PORT=16399
T3_FIXTURE_LOCAL_PORT=16300`, and `start <name> --port 16310`.

### Mark unread on a completed thread

With `seed second --turns`, "Background thread (completed turn)" is read: its row shows no
status. Mark unread (its context menu) rewinds the server's visit watermark to just before the
completion (`thread.mark-unread`), and the row shows the reference's green "Completed" status
(Sidebar.logic.ts `hasUnseenCompletion`); opening the thread reads it again. The menu needs real
input (below); the row's answer to the command can be driven by sending it through the server's
API while the app runs.

What an agent drive cannot do here: choose an item of a native context menu (under the agent it
answers dismissed: nothing is chosen and its items are not logged). That row needs real input. A
file drop on a sidebar row is the row's Contract `drop`: `tap thread-<id> drop <path…>`.

## Real input (a normal launch)

A normal launch saves the pairing and its token in the connection catalog, one Keychain item (service the
app's bundle id, account `t3-catalog`). Launch a lane copy
of the bundle (its own bundle id) with `T3_LOCAL_HOME=<root>/local/t3home T3_LOCAL_PORT=16500`,
pair with a URL from `bun -e 'import {fixturePaths,pairingUrl} from "./lib.mjs"; await Bun.write("/abs/pairing-url", pairingUrl(fixturePaths(), "second"))'`
(paste it from that file), and afterwards remove it with `bun env.mjs forget second --app <the copy's bundle id>`
(only that fixture's entry and tokens; "none saved" when there was none).
