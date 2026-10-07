# SSH password and remote editor evidence

Captured 2026-10-06 in the isolated `com.exact.t3code.ssh157` macOS app.
The fixture runs the unmodified T3 server at reference `1e2ecbd975`.
SSH uses the local Docker OpenSSH host; the local project lives under
`target/ssh-live/local-project`. The Markdown/FIFO captures precede the alias fix; the separately labelled alias
captures verify implementation commit `762f501cb`.

| Interaction | Observed result | Evidence |
| --- | --- | --- |
| Concurrent password prompts | `t3-ssh157` is visible with one queued request. After dismissal, `oracle-second` is visible with no queued request and an empty secure field. | [First screenshot](native-fifo-first.png), [first AX](native-fifo-first-ax.txt), [second screenshot](native-fifo-second.png), [second AX](native-fifo-second-ax.txt), [state](native-fifo-result.json) |
| Responding state | Of 239 samples over about one second, two captured the visible loading state; both had Continue disabled. | [Sample summary](loading-summary.json) |
| Remote Markdown menu | The actual native context menu offers Copy relative path and Copy full path, with no editor or Finder actions. Copy relative path produced `README.md`. | [Screenshot](markdown-remote-menu.png), [AX](markdown-remote-menu-ax.txt), [clipboard assertion](markdown-remote-copy.json) |
| Local Markdown menu | The actual native context menu offers Open in Cursor, Reveal in Finder, and both copy actions. | [Screenshot](markdown-local-menu.png), [AX](markdown-local-menu-ax.txt) |
| Local Markdown reveal | Choosing Reveal in Finder opened the fixture directory with `README.md` selected. | [Finder screenshot](markdown-local-reveal.png), [filtered AX](markdown-local-reveal-ax.txt) |

The two Markdown messages were inserted into isolated projection tables, without
calling an LLM provider. Authenticated `getThreadProjection` returned each message
and its visible turn item before the native drive. The synthetic conversation
message includes the required `creationSource: "server"` field. The remote
provider warning in its screenshot is expected: no provider was installed or used.

The FIFO fixture delays BatchMode SSH attempts by 35 seconds to overlap requests,
then invokes real `/usr/bin/ssh`. Its `afterCancel` sample still shows the second
prompt; it does not prove that both prompts were dismissed. The loading samples
prove an observed disabled state, not its exact duration.

Only fixture screenshots, accessibility text and reduced assertions are retained.
Tokens, pairing responses, database files and helper scripts remain untracked.
The Finder AX excerpt omits its sidebar. No unrelated Code editor release content
is included.

## Pinned oracle and earlier acceptance evidence

The desktop oracle was built from reference `1e2ecbd975` with Electron 44.4.2
and Node 24.21.0. Fixture-only changes redirect its app data and SSH discovery to
isolated directories and its SSH command to `/usr/bin/ssh -F <fixture config>`.
Password authentication, queue service and renderer were unchanged. No reusable
oracle helper or fixture credentials are committed here.

| Interaction | Observed result | Evidence |
| --- | --- | --- |
| Korean input-source comparison | After selecting Korean 2-Set, focusing either password field selected ABC. Physical G yielded `g` in the oracle; native AX remained masked. This matches the secure fields' observed behavior, not general Korean text input. | [Comparison](ime-comparison.json), [oracle field](oracle-password-light.png), [native field](clone-ime.png), [native AX](clone-ime-ax.txt) |
| Oracle FIFO comparison | The rendered prompt changed from `oracle-first` to `oracle-second` after dismissal. The native capture above shows the same first-to-second transition. | [Oracle first](oracle-fifo-first.png), [oracle second](oracle-fifo-second.png), [oracle dialog text](oracle-fifo.json) |
| Native expiry | The earlier real three-minute wait reached Expired, the error hint, disabled input/Continue and Dismiss. The expiry seam was not shortened. | [Screenshot](real-expiry.png), [AX](real-expiry-ax.txt) |
| Remote project in a real editor | VS Code's window names `project [SSH: t3-ssh157]`, and the opened README contains `# SSH acceptance project`. This proves the earlier project Open outcome, not the later Files-specific launch. | [Editor screenshot](real-editor-project.png), [AX](real-editor-project-ax.txt) |
| Secret audit at the earlier checkpoint | 80 checked fixture product-storage/log/AX files had zero matches, as did the sampled unified log. This is bounded evidence, not a forensic Keychain audit. | [Audit counts](secret-audit.json) |
| Saved-alias regression | Remembering a newly added alias before the first picker previously skipped loading saved tunnels. The new test failed with local-exec and no editors, then passed after separating cache completeness from remembered aliases. | [40 focused passing tests](remaining-alias-cache-tests.log), [strict TS](remaining-alias-cache-tsc.log), [caps](remaining-alias-cache-caps.log) |
| Remote Files handoff | Choosing VS Code reached its actual external-application confirmation with the requested SSH alias and README path. The reference also passes the absolute file path to the remote URL builder; editor handling beyond dispatch is separate. | [OS confirmation AX](files-remote-handoff-ax.txt) |
| Local Files Open | Choosing the offered Finder entry completed the local shell RPC; Finder showed the fixture README selected. The window was already open, so no new-window transition is claimed. | [Filtered Finder AX](files-local-finder-ax.txt), [successful backend requests](local-open-rpc-results.json) |
| Rebuilt alias fix | On `762f501cb`, the native app authenticated saved tunnels and selected the SSH conversation. Open is enabled at `/home/tester/project`, with Cursor, VS Code and Zed available. | [Screenshot](alias-fixed.png), [AX](alias-fixed-ax.txt), [reduced state](alias-fixed-summary.json) |

`oracle-fifo.json` records `remainingDialogs: 1`. Like the native second-prompt
capture, it proves the transition, not a fully drained queue. No durable oracle
expired-state capture was retained, so the expiry row above is native-only.

## Dependency scope and remaining work

PR [#142](https://github.com/ccheever/exact2/pull/142) merged at `9670b0723`
and is included by merge commit `bd59d77d7`. Its
[feature acceptance record](../20261005-remote-scopes-and-update-commands/20261006-repair-and-trace/README.md)
uses a pinned direct Electron fixture and reports its actual protocol differences
rather than claiming full RPC equality. Here, direct oracle evidence covers the
SSH prompt and input behavior. It does not complete the separately planned generic
oracle/trace tooling or clone-on-main task, nor claim the parent's full T0 matrix.
Those prerequisite records remain planned/unverified.

Local `shell.openInEditor` calls completed successfully in the fixture backend,
but the observed Code process used a separate user-data directory from the CLI's
default profile. Its unchanged window cannot establish local file-opening success.
Files also displays the first available editor independently of the remembered
selection. No VS Code document-view success is claimed for that local attempt. The supported
local Open outcome uses the offered Finder entry, recorded above.

## Final missing-route acceptance

The user authorized a temporary HTTPS tunnel to the isolated authenticated backend.
The real server config advertised no SSH targets. The native client paired over
HTTPS and opened the fixture thread and README through its normal UI.

- Details Open was disabled, and its menu displayed a disabled “No SSH route” item.
- Files Open was disabled, and its menu displayed the same unavailable explanation.
- After Escape closed the Files menu, Cmd+O left the native window in place with no
  open request. This observation complements the unavailable-action no-op unit test;
  it is not a system-wide audit of every possible side effect.

Evidence: [details screenshot](unavailable-details.png), [details AX](unavailable-details-ax.txt),
[Files screenshot](unavailable-files.png), [Files AX](unavailable-files-ax.txt),
[shortcut AX](unavailable-shortcut-ax.txt), [assertions](unavailable-summary.json).

All feature acceptance follow-up items are now covered. Generic parent oracle/trace
infrastructure remains outside this feature's completion claim. The temporary
public tunnel is terminated after this drive; no pairing credential is retained here.

[HTTPS config](route-https-check.json) records the empty advertised routes;
[cleanup](route-cleanup.json) confirms the tunnel, proxy and dedicated backend exited.
