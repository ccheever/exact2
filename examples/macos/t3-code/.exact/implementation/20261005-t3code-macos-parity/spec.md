# 20261005-t3code-macos-parity specification

## Requested outcome

A macOS desktop app, the Exact2 example `examples/macos/t3-code`, that behaves like the
T3 Code desktop app (Electron + React UI + bundled Node server). The audience is a T3 Code
user on a Mac. The clone already covers most of the client surface (see the clone's
`README.md` and `AGENT-HANDOFF.md`). This plan covers the remaining work: finishing round 12,
moving the clone onto current exact2 `main`, the gaps found by the 2026-10-05 audit, and the
verification that proves parity against the reference desktop app.

User request (2026-10-05): "t3code를 지금 examples에 macos 버전을 구현하고 있다. 읽어보고 필요
작업 계획 세워" — read the current state and plan the required work.

## Confirmed requirements

| Decision | Agreed value | Evidence or user decision |
| --- | --- | --- |
| Target platforms | macOS desktop only (Apple Silicon Mac, macOS 14 or later per `app.json`). Default window 1280×840, minimum 840×620. Light and dark appearance. | `app.json` `host.macos`; clone README "Web, iOS and Linux are outside this example's scope"; user decision 2026-10-05 |
| Goal and logic reuse | Re-implement the Electron T3 Code desktop app with exact2. Data, client and server logic come from T3 Code itself: port the reference modules (for example `packages/client-runtime`, `packages/shared`, the `*.logic.ts` files) with their names and tests, and change them only where exact2 requires it; such a change, or an improvement it needs, is recorded in the file header. The server is not re-implemented. | User message 2026-10-05: "eletron으로 구현된 t3code를 exact2로 재구현하는거야 … t3code에서 사용한걸 동일하게 사용하되 exact에 맞게 수정이 필요하면 개선되야지" |
| Unsupported capability | When a feature needs something exact2 does not support, file it as an **issue only** (`issues/YYYYMMDD-*.md`, published through `issue-open` after the user approves). No framework PR comes from this plan. Each issue explains in detail why it arose (T3 behavior, exact2 behavior with evidence, the clone's workaround) and why it must be resolved (parity impact). The goal is a complete clone, so a declared deviation is not an end state: every issue ends resolved upstream and adopted in the app (verified by `issue-close`), or closed by a user decision. Meanwhile a ticket may ship a workaround and records how it differs. | User messages 2026-10-05: "구현이 필요한데 exact2에서 지원을 안하면 그건 issue로 기록해야지"; "issue만 올려. 대신 컨텍스트를 자세하게 제공해서 왜 그 issue가 발생했고, 왜 해결되야하는지 적어야함"; "exact2 framework level 에서 구현 안되는건, issue로 남겨서 support 되게 만들어야지" |
| Feature scope | Every reference desktop feature, except the exclusions below. Includes the embedded local T3 server: "This machine", Local environment switch, Network access, Tailscale HTTPS, authorized clients, pairing links; and remote environments (pairing, SSH, multiple routes). | User decision 2026-10-05 #2 and #5; answer 2026-10-05: "모든 기능을 다 만들어야해" |
| Delivery | No deployment: no store, update feed, Exact delivery streams or hosted web root. The build output, a downloadable archive of the `.app`, must run on another person's Apple Silicon Mac (macOS 14 or later) that has no exact2 repo, no T3 Code checkout, no Bun, no Node and no T3 installation. Provider CLIs (Codex, Claude, …) are the recipient's own, as in the reference. | Answer 2026-10-05: "배포는 제외하고, build해서 다른 사람한테 다운로드 파일 주면 그 사람이 쓸 수 있어야함" |
| Visual and interaction fidelity | Match the reference **desktop** app. The oracle is a build of the reference Electron app at the pinned reference commit; the server's web client remains a secondary pixel oracle. A difference caused by an exact2 limit is declared in `EXACT2-GAPS.md` with evidence; it is not silently accepted. | User decision 2026-10-05 #4; the audit found earlier rounds used the web client as the oracle |
| Backend and data | T3 server from the reference runtime. Local: embedded server that shares `~/.t3` with the original (only one of the clone and T3 Code (Nightly) runs at a time). Remote: paired environments over HTTP + WebSocket RPC, and SSH environments. Credentials in Keychain. App preferences in the versioned `t3-code.json` under Exact's app data folder. GitHub features go through the server's `gh`; verified against a fake `gh` now, live with disposable accounts later. | User decisions 2026-10-05 #2, #4 and the 2026-10-05 evening decision (shared `~/.t3`); clone README |
| Embedded server source and self-contained build | The embedded server is the same server that the original desktop app ships (same version and build, not a port or a rebuild with changes). Building and running the example must not need the reference checkout. The example's own build step gets the pinned server runtime (and any Node runtime it needs) from an official published source with checksum verification, or from files vendored into the example with provenance and license. The same rule applies to every other reference-derived runtime asset (for example the terminal emulator files). The reference checkout is used only for verification (oracle builds, test porting, protocol traces). | User message 2026-10-05: "내장 서버는 원본앱에 있는거 그대로 쓰는게 맞는데, 원본 repo 없이 example에서 앱 빌드하고 실행하면 되야해" |
| Embedded server artifact (U3) | The official CLI archive (Node SEA) at the release that matches the reference pin, fetched at build time and verified against the release `SHA256SUMS` and a hash committed in the example; unpacked under `<T3 home>/runtime/versions`. Same version and commit as the desktop app's server; packaged as a SEA instead of Electron-as-Node. | User decision 2026-10-05 ("권장대로해") |
| Settings that restart the local server (U4) | The same as T3 Code: relaunch the whole app after a Local environment, Network access or Tailscale change. If exact2 cannot relaunch an app (issue X45), a restart-in-place stopgap ships and the relaunch rows stay blocked until X45 is resolved and adopted. | User decision 2026-10-05 ("t3code 원본과 동일하게해") |
| Download signing and test (U11) | Ad-hoc signature, zip archive; tested on a clean macOS 14 VM (the stated minimum). The recipient opens the app once through Gatekeeper ("Open Anyway"). | User decision 2026-10-05 ("권장대로해") |
| Verification tools (U23) | Committed under `examples/macos/t3-code/tools/` with the lane tools' relative paths, so a fresh clone can run every acceptance command; no absolute user paths or credentials. | User decision 2026-10-05 ("tools/에 커밋") |
| Target repositories | Application: `https://github.com/ccheever/exact2`, integration branch `daehyeon/t3-code` (pushed to origin; one PR per ticket into it). Framework: no PRs from this plan; issues only, filed on the same repository. Reference (read-only): T3 Code checkout at `1e2ecbd975`. | User decisions 2026-10-05 (integration branch; issues only) |
| Framework boundary | App work stays under `examples/macos/t3-code/` (plus root workspace registration). Kernel, runner, host, contract compiler, js and scripts do not change for the example; missing support is an issue. | Memory "examples leave the framework alone" (2026-10-01); 2026-10-05 "issue만 올려" |
| Execution | One multi-agent workflow per phase, at most 9 agents (up to 8 lanes plus 1 integrator). Lanes never use port 3773, the real `~/.t3`, or the `t3code` URL scheme. Every T3 server the plan starts (lane, oracle, embedded) sets `T3CODE_TELEMETRY_ENABLED=false`, because telemetry is excluded (issue X39). | User decision 2026-10-05 #6; clone `AGENT-HANDOFF.md` "Safety notes" |

## Target matrix

| OS / version | Exact host / renderer | Phone / tablet / desktop | Window sizes / orientation | Input / accessibility | Required runtime and performance checks |
| --- | --- | --- | --- | --- | --- |
| macOS 26.6.2 (build 25G83) on this Mac, Xcode 27.0; app minimum macOS 14 | macOS AppKit host, bundled app (`bun host/apple/build.mjs macos-t3-code-apple --bundle --run` with `EXACT_APP_DIR`; agent `macos`) | Desktop | 1280×840 and 840×620 (minimum); live resize; full screen; light and dark | Trackpad and mouse (click, right-click, hover, drag, wheel), keyboard (US and Korean 2-Set IME, chords, ⌘Q hold), `aria-label` on icon buttons, keyboard focus in dialogs; VoiceOver spot checks only where a ticket names them | Agent drives (`tree`, `state`, `layout`, screenshots, `clock`); the clone's AppKit test binaries; pixel pairs against the desktop oracle at both sizes and both appearances; protocol trace comparison for RPC-using features; effect checks (server state, git, files, fake `gh` calls); attended real-input sessions for inputs the agent cannot send; `perf` only where a ticket names a workload |

The library documents macOS resize, keyboard/focus, pointer/menu, native controls,
storage/relaunch and window lifecycle as macOS verification priorities (`platforms.md`).
A web run does not prove macOS behavior.

## Already implemented — excluded from this plan

The user asked (2026-10-05) that work already done stays out of the plan. These workflows
were implemented and checked in rounds 1–11 (round 11: `bun test` 991/0, strict `tsc`
clean, `contract build` OK, 26 AppKit test binaries green, pixel matrix at 1280×840 and
840×620 in light and dark; source: clone `AGENT-HANDOFF.md` "Checks on the integrated tree"
and the committed `STATUS.md` §1). Tickets do not re-implement them. A ticket touches one only
to fill a gap marked **partial** in [research](research.md), and then names the existing code
it reuses.

| Area | Implemented workflows |
| --- | --- |
| Connection | Pairing, Keychain credentials, saved environments, reconnect ladder, SSH environments, outdated-host update, Load balancing / GitHub sharing settings (UI) |
| Sidebar | Projects, shelves (Working / active / Settled / Snoozed), search, pin, settle and snooze, drag between shelves, row-action sweep, draft rows with context menu and undo |
| Conversation | History paging, streaming rows, work log, checkpoints, changed files, Markdown, code blocks, Mermaid, Shiki-matched code colours, workspace-preparation Retry |
| Composer | Send / stop / queue / steer, model picker, reasoning and runtime options, Plan / Build, slash and @ menus, file / image / video attachments, stash, multi-model drafts |
| Requests | Approvals and questions |
| Version control | Branch picker, git actions, commit dialog, pull, publish wizard, new worktree, pull request checkout dialog |
| Right panel | Workspace card, diffs, Files (tree, previews, editor, rendered HTML), linked pull requests, pull request surface (read, Ready / Merge), attachment previews (PDF, HTML, audio, video), Device surface (3D phone, tools, iPhone Duo, foldable) |
| Pages | Pull Requests list and detail (read), Usage, welcome wizard, command palette, toasts, notifications |
| Settings | All 14 routes, theme editor and VS Code theme import, keybindings, providers (configuration only), connections, SnapShots, diagnostics, licenses. Note: the Background activity switches "Pause when host is locked" and "Pause on battery" show but have no effect while telemetry part 3 (the host pipes) is off — issue X39 |
| Desktop shell | Reference menu bar, ⌘Q hold, window frame persistence, Korean 2-Set chord handling |
| Round 12 (done lanes) | r12-sidebar and r12-render (`bun test` 1025/0 after the stop; native build and Swift tests not yet run — finished in ticket `20261005-round12-wrapup`) |

The research table marks each inventory item missing / partial / done with clone file:line
evidence. Items found **done** are listed there and get no ticket.

## Exclusions

- Not implemented now, tracked as **blocked tickets with issues** (implemented or closed after
  the issue is resolved or decided):
  - Browser surface (Chromium + CDP) — `20261005-browser-surface`, issue X1 (Charlie's waiver
    of the DEFERRED browser-shell rule and a chosen path; path A also needs framework support). Until then links open in the
    system browser.
  - View › Toggle Developer Tools — `20261005-app-developer-tools`, issue X2 (DEFERRED "no
    devtools UI").
  - T3 Connect / Clerk sign-in, relay connections and the `t3code://` handoff —
    `20261005-t3-connect-sign-in`, issue X38.
  - Telemetry — `20261005-telemetry`, issue X39.
  - The T3 update feed — `20261005-app-update-feed`, issue X40.
  - WSL environments (Windows only) — `20261005-wsl-environments`, issue X41.
- Deployment: App Store or other store listings, an auto-update channel, Exact delivery
  (`scripts/deploy.mjs`) and a hosted web root. A downloadable build that works on another
  Mac is **in** scope (see Delivery).
- Web, iOS, Linux and Windows builds of this example.
- Live GitHub write verification (deferred to disposable accounts; fake `gh` now).

## Acceptance

The plan is complete when every ticket is verified and merged into `daehyeon/t3-code` (or, for
the six blocked tickets, closed by the user's decision on its issue), every issue in
`issues/` is closed — resolved upstream and adopted in the app (`issue-close`), or closed by a
decision — and the integrated acceptance in [plan](plan.md) passes on the target above. In short:

1. A fresh launch with a shared `~/.t3` starts the embedded server, shows "This machine", and
   lets the user add a project, start a thread, stream a reply, answer an approval, and see
   the diff — with the reference desktop app's look at both window sizes in light and dark.
2. A remote environment pairs, survives a server restart and an app relaunch, and keeps
   credentials in Keychain only.
3. Pull request, provider, settings, terminal, device-player and desktop-shell workflows match
   the reference desktop app in pixel pairs, protocol traces and effect checks. A difference
   caused by an exact2 limit is allowed only while its issue is open; the plan ends with every
   issue closed (resolved and adopted, or closed by a decision).
4. Failure states (lost connection, refused request, uncertain write, missing provider,
   server crash) show the reference's wording and recovery paths.
5. Real-input checks that the agent cannot send pass in attended sessions, with recorded
   steps and evidence.

## Open decisions

| Question | Affected work | Options |
| --- | --- | --- |
| Framework knowledge for app-native code (decide before the first `prepare`). The bundled library does not cover app-local Swift modules, hooks, native views, data-module networking, bundle assets, URL schemes, menus, window chrome or notifications (all "unknown in this library"). | Every ticket that adds native code (most of them) | (a) Run `update-best-practices` for those topics before `prepare`; (b) accept the clone's own runtime evidence on the pinned `main` (from ticket `20261005-clone-on-exact2-main`) as the feasibility basis, ticket by ticket. |
| Apparatus approval. CLAUDE.md: agents add no apparatus without a human saying so. | `20261005-desktop-oracle-and-trace`, `20261005-fake-github-fixture`, `20261005-embedded-server-runtime` (runtime staging), `20261005-terminal-surface` (vendor staging), `20261005-portable-app-download` (packaging script) | Approve the apparatus listed in [plan](plan.md) "Apparatus requiring approval", per item. |
| Tailscale availability on the verification Mac. | `20261005-this-machine-network-access` verification | (a) A tailnet the user provides; (b) verify the Tailscale rows with a stub endpoint provider only and mark live Tailscale unverified. |
| `t3code://` deep link (E4). Its handler in the reference belongs to the Clerk bridge (hosted-web Codex handoff), which the spec excludes. Codex sign-in on the desktop uses a loopback listener, not the scheme. | `20261005-app-activation` | (a) Exclude with T3 Connect / Clerk; (b) build it (needs issue X5). |
| Real-input checks. The agent cannot send right-click, real hover/drag, real wheel, IME or OS prompts. | Most UI tickets | (a) Batch attended sessions per phase (as rounds 4–11 did); a ticket with an open attended row stays `verification: blocked` until its session passes, so its PR waits (recommended); (b) run them per ticket when computer-use tools are available in the session. |
| Reference pin refresh. The reference moved 53 commits in one day; this plan pins `1e2ecbd975` (no fetch was made). | Every upstream-sync ticket | Fetch the reference at each sync ticket's `prepare` (user or approved command) and record the new pin. |
| Policy and scope issues: X1 Browser, X2 DevTools, X19 timers, X28 notification actions (DEFERRED rules; Charlie's waiver), X38 T3 Connect/Clerk, X39 telemetry, X40 update feed, X41 WSL. | The blocked tickets and the rows that carry these differences | Decided per issue after `issue-open` publishes it; each blocked ticket is then implemented or closed. |
