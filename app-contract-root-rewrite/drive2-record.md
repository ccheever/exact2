# app-contract-root-rewrite: extra drive record (coordinator's go-ahead, 2026-10-09)

Usage and the thread title menu only, base `b53cd7da7` (evidence-base worktree under its .build-lock, released after) and branch `a3b9b094f`, same script (target/arr/drive2.mjs, scratch), same lane as the first drive
(isolated env under target/arr/lane; local server 16610 branch / 16612 base; the GitHub lane's primary server on 16620, pid 59156, stopped after; one single-use pairing link per run, never printed).
One titled thread was created on the lane server by RPC before the runs (`orchestration.dispatchCommand` `thread.create`, title “Review the usage headings”, the sandbox project); the rename was cancelled with Escape, so its title is unchanged.

Steps: Welcome pairing and skip import (as the first drive, no screenshots); Usage from the home page's sidebar; Limits; the seeded thread's row; its title; the title menu; Rename thread; Escape.

## branch run
- {"t": "2026-10-08T15:48:22.694Z", "step": "launched", "appPid": 59486, "which": "after", "port": "16610"}
- {"t": "2026-10-08T15:48:24.104Z", "step": "ready", "what": "welcome and the local server", "ms": 1410}
- {"t": "2026-10-08T15:48:27.787Z", "step": "type", "id": "welcome-pairing-link", "text": "<pairing URL, not logged>"}
- {"t": "2026-10-08T15:48:28.407Z", "step": "ready", "what": "a paired computer", "ms": 6}
- {"t": "2026-10-08T15:48:31.123Z", "step": "ready", "what": "agents step", "ms": 6}
- {"t": "2026-10-08T15:48:31.480Z", "step": "ready", "what": "projects step", "ms": 5}
- {"t": "2026-10-08T15:48:31.912Z", "step": "ready", "what": "welcome closed", "ms": 6}
- {"t": "2026-10-08T15:48:35.664Z", "step": "ready", "what": "usage page", "ms": 6}
- {"t": "2026-10-08T15:48:59.693Z", "step": "timeout", "what": "a usage segment", "ms": 20000}
- {"t": "2026-10-08T15:49:01.608Z", "step": "segment", "seg": null}
- {"t": "2026-10-08T15:49:01.615Z", "step": "ready", "what": "the seeded thread row", "ms": 7}
- {"t": "2026-10-08T15:49:02.131Z", "step": "ready", "what": "the thread title", "ms": 6}
- {"t": "2026-10-08T15:49:04.899Z", "step": "ready", "what": "the title menu", "ms": 7}
- {"t": "2026-10-08T15:49:06.461Z", "step": "ready", "what": "the rename field", "ms": 7}
- {"t": "2026-10-08T15:49:08.010Z", "step": "ready", "what": "the rename field gone", "ms": 6}
- {"t": "2026-10-08T15:49:09.219Z", "step": "result", "ready": true, "paired": true, "welcomeClosed": true, "usage": true, "row": true, "thread": true, "menu": true, "renaming": true, "renameCancelled": true}
- {"t": "2026-10-08T15:49:09.240Z", "step": "closed", "appPid": 59486}

## base run
- {"t": "2026-10-08T15:53:56.327Z", "step": "launched", "appPid": 70424, "which": "base", "port": "16612"}
- {"t": "2026-10-08T15:53:57.415Z", "step": "ready", "what": "welcome and the local server", "ms": 1085}
- {"t": "2026-10-08T15:54:01.094Z", "step": "type", "id": "welcome-pairing-link", "text": "<pairing URL, not logged>"}
- {"t": "2026-10-08T15:54:01.720Z", "step": "ready", "what": "a paired computer", "ms": 6}
- {"t": "2026-10-08T15:54:04.433Z", "step": "ready", "what": "agents step", "ms": 7}
- {"t": "2026-10-08T15:54:04.793Z", "step": "ready", "what": "projects step", "ms": 5}
- {"t": "2026-10-08T15:54:05.240Z", "step": "ready", "what": "welcome closed", "ms": 6}
- {"t": "2026-10-08T15:54:09.032Z", "step": "ready", "what": "usage page", "ms": 7}
- {"t": "2026-10-08T15:54:33.060Z", "step": "timeout", "what": "a usage segment", "ms": 20000}
- {"t": "2026-10-08T15:54:34.977Z", "step": "segment", "seg": null}
- {"t": "2026-10-08T15:54:34.985Z", "step": "ready", "what": "the seeded thread row", "ms": 7}
- {"t": "2026-10-08T15:54:35.473Z", "step": "ready", "what": "the thread title", "ms": 7}
- {"t": "2026-10-08T15:54:38.235Z", "step": "ready", "what": "the title menu", "ms": 6}
- {"t": "2026-10-08T15:54:39.798Z", "step": "ready", "what": "the rename field", "ms": 6}
- {"t": "2026-10-08T15:54:41.342Z", "step": "ready", "what": "the rename field gone", "ms": 7}
- {"t": "2026-10-08T15:54:42.547Z", "step": "result", "ready": true, "paired": true, "welcomeClosed": true, "usage": true, "row": true, "thread": true, "menu": true, "renaming": true, "renameCancelled": true}
- {"t": "2026-10-08T15:54:42.569Z", "step": "closed", "appPid": 70424}

## Screens, base against branch

| Screen | Differing pixels |
| --- | --- |
| 21-usage.png | 0.000% |
| 21b-usage-limits.png | 0.000% |
| 24-thread.png | 0.000% |
| 25-title-menu.png | 0.000% |
| 26-renaming.png | 0.000% |
| 27-rename-escaped.png | 0.000% |

Not done: pinning a Usage segment's popover. The lane has no signed-in provider, so the page has no segment (Limits: “Codex: Could not read limits.”), in base and branch alike.
