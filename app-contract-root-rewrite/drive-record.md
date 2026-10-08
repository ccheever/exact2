# app-contract-root-rewrite: live drive record (agent mode, macOS, 1280x840)

Same script, same lane, base then branch: target/arr/drive.mjs (scratch). App: T3 Code (Exact) development bundle, launched by scripts/agent.mjs `open({host: 'macos'})`
with an isolated env (HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, TMPDIR under the worktree's target/arr/lane; T3_LOCAL_HOME fresh per run; T3_LOCAL_PORT 16610 branch / 16612 base;
PATH /usr/bin:/bin:/usr/sbin:/sbin plus an empty lane bin; T3CODE_TELEMETRY_ENABLED=false). Pairing: the real-GitHub lane's primary server (daehyeonmun2021, playground
sandbox) started in this worktree on port 16620 (pid 64211, stopped after the drives); one single-use pairing link per run (`lane.mjs pair primary`), read from its
0600 file and typed into Welcome's pairing link field; never printed, logged or screenshotted. Agent drives keep no Keychain item.

Builds: base = evidence-base worktree at 96c4c38f2 (under its .build-lock, released after); branch = this branch at cbdc478ba (the rewrite before the #329 merge; the merge changes no moved code).

## branch run
- {"t": "2026-10-08T14:49:59.841Z", "step": "launched", "appPid": 68562, "which": "after", "port": "16610"}
- {"t": "2026-10-08T14:50:00.381Z", "step": "ready", "what": "welcome and the local server", "ms": 540}
- {"t": "2026-10-08T14:50:04.892Z", "step": "type", "id": "welcome-pairing-link", "text": "<pairing URL, not logged>"}
- {"t": "2026-10-08T14:50:05.506Z", "step": "ready", "what": "a paired computer", "ms": 6}
- {"t": "2026-10-08T14:50:09.323Z", "step": "ready", "what": "agents step", "ms": 6}
- {"t": "2026-10-08T14:50:11.584Z", "step": "ready", "what": "projects step", "ms": 6}
- {"t": "2026-10-08T14:50:13.927Z", "step": "ready", "what": "welcome closed", "ms": 6}
- {"t": "2026-10-08T14:50:17.864Z", "step": "ready", "what": "settings", "ms": 16}
- {"t": "2026-10-08T14:50:18.274Z", "step": "ready", "what": "connections page", "ms": 8}
- {"t": "2026-10-08T14:50:20.526Z", "step": "ready", "what": "Add environment", "ms": 7}
- {"t": "2026-10-08T14:50:20.952Z", "step": "ready", "what": "Add environment closed", "ms": 8}
- {"t": "2026-10-08T14:50:21.355Z", "step": "ready", "what": "providers page", "ms": 8}
- {"t": "2026-10-08T14:50:23.595Z", "step": "type", "id": "settings-search", "text": "theme"}
- {"t": "2026-10-08T14:50:24.415Z", "step": "ready", "what": "settings closed", "ms": 6}
- {"t": "2026-10-08T14:50:26.628Z", "step": "ready", "what": "composer", "ms": 5}
- {"t": "2026-10-08T14:50:27.003Z", "step": "type", "id": "composer", "text": "Checking the window after the root rewrite"}
- {"t": "2026-10-08T14:50:27.089Z", "step": "no thread-title"}
- {"t": "2026-10-08T14:50:49.865Z", "step": "ready", "what": "pull request rows", "ms": 20}
- {"t": "2026-10-08T14:50:53.003Z", "step": "ready", "what": "pull request panel", "ms": 23}
- {"t": "2026-10-08T14:51:14.230Z", "step": "tap-failed", "id": "open-usage", "message": "no view matches open-usage by testId, label or text; targets here: toggle-sidebar, sidebar-brand, search-threads, filter-project, add-project, new-thread, sidebar-back, chat-header, header-project, to"}
- {"t": "2026-10-08T14:51:34.536Z", "step": "timeout", "what": "usage page", "ms": 20000}
- {"t": "2026-10-08T14:51:36.487Z", "step": "result", "ready": true, "paired": true, "welcomeClosed": true, "settingsClosed": true, "composer": true, "prRows": true, "prPanel": true, "usage": false}
- {"t": "2026-10-08T14:51:36.520Z", "step": "closed", "appPid": 68562}

## base run
- {"t": "2026-10-08T14:54:00.278Z", "step": "launched", "appPid": 76226, "which": "base", "port": "16612"}
- {"t": "2026-10-08T14:54:01.439Z", "step": "ready", "what": "welcome and the local server", "ms": 1160}
- {"t": "2026-10-08T14:54:06.038Z", "step": "type", "id": "welcome-pairing-link", "text": "<pairing URL, not logged>"}
- {"t": "2026-10-08T14:54:06.664Z", "step": "ready", "what": "a paired computer", "ms": 6}
- {"t": "2026-10-08T14:54:10.556Z", "step": "ready", "what": "agents step", "ms": 9}
- {"t": "2026-10-08T14:54:12.838Z", "step": "ready", "what": "projects step", "ms": 10}
- {"t": "2026-10-08T14:54:15.240Z", "step": "ready", "what": "welcome closed", "ms": 9}
- {"t": "2026-10-08T14:54:19.173Z", "step": "ready", "what": "settings", "ms": 14}
- {"t": "2026-10-08T14:54:19.582Z", "step": "ready", "what": "connections page", "ms": 8}
- {"t": "2026-10-08T14:54:21.874Z", "step": "ready", "what": "Add environment", "ms": 12}
- {"t": "2026-10-08T14:54:22.323Z", "step": "ready", "what": "Add environment closed", "ms": 7}
- {"t": "2026-10-08T14:54:22.710Z", "step": "ready", "what": "providers page", "ms": 14}
- {"t": "2026-10-08T14:54:24.981Z", "step": "type", "id": "settings-search", "text": "theme"}
- {"t": "2026-10-08T14:54:25.826Z", "step": "ready", "what": "settings closed", "ms": 7}
- {"t": "2026-10-08T14:54:28.050Z", "step": "ready", "what": "composer", "ms": 6}
- {"t": "2026-10-08T14:54:28.426Z", "step": "type", "id": "composer", "text": "Checking the window after the root rewrite"}
- {"t": "2026-10-08T14:54:28.516Z", "step": "no thread-title"}
- {"t": "2026-10-08T14:54:36.352Z", "step": "ready", "what": "pull request rows", "ms": 21}
- {"t": "2026-10-08T14:54:39.408Z", "step": "ready", "what": "pull request panel", "ms": 23}
- {"t": "2026-10-08T14:54:56.377Z", "step": "clock-error", "message": "clock: the clock cannot go backwards (38900.0 \u2192 38899.99999999999)"}
- {"t": "2026-10-08T14:55:00.718Z", "step": "tap-failed", "id": "open-usage", "message": "no view matches open-usage by testId, label or text; targets here: toggle-sidebar, sidebar-brand, search-threads, filter-project, add-project, new-thread, sidebar-back, chat-header, header-project, to"}
- {"t": "2026-10-08T14:55:21.293Z", "step": "timeout", "what": "usage page", "ms": 20000}
- {"t": "2026-10-08T14:55:23.268Z", "step": "result", "ready": true, "paired": true, "welcomeClosed": true, "settingsClosed": true, "composer": true, "prRows": true, "prPanel": true, "usage": false}
- {"t": "2026-10-08T14:55:23.295Z", "step": "closed", "appPid": 76226}

## Attempts
- Branch attempt 1 (14:48Z): stopped at its first step: the fresh home opens the Welcome wizard, which makes the sidebar inert (the script expected the sidebar). Nothing was paired; the pairing link was never typed.
- Branch attempt 2 (the one retry, 14:50Z) and the base run (14:54Z): the steps below, identical results.

## Screens, base against branch (pixels differing by more than 24/255)

| Screen | Differing pixels | Note |
| --- | --- | --- |
| 01-welcome.png | 0.000% | identical |
| 02-welcome-help.png | 0.000% | identical |
| 03-welcome-paired.png | 0.000% | identical |
| 04-welcome-agents.png | 0.036% | a hover highlight on “Use existing CLI” in the branch shot only (the re-hover under the resting agent pointer; no layout change) |
| 05-welcome-projects.png | 0.000% | identical |
| 06-home.png | 0.000% | identical |
| 07-connections.png | 0.000% | identical |
| 08-add-environment.png | 0.000% | identical |
| 09-providers.png | 0.000% | identical |
| 10-settings-search.png | 0.000% | identical |
| 11-composer.png | 0.000% | identical |
| 13-pr-list.png | 1.321% | live GitHub data: list order and the loaded diff stats changed between the runs |
| 14-pr-summary.png | 1.836% | live GitHub data (the list column behind the panel) |
| 15-pr-code.png | 1.358% | live GitHub data (the list column) |
| 16-pr-timeline.png | 1.633% | live GitHub data (the list column) |
| 17-usage.png | 1.836% | not the Usage page: the pull request page's sidebar shows Back, not the Usage button (script); same in both runs |

Not reached in either run (script, the same in both): Usage (the pull request page's sidebar shows Back in place of the footer buttons) and the thread title's menu (a draft thread's title has no `thread-title` test id).
