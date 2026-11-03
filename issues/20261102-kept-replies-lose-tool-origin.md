# Kept request replies lose their tool origin and admit restricted work

**Status:** Closed
**Severity:** P1
**Systems:** Runner origin admission, `runner/src/runner/module_work.rs`
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

A tool-origin request that is superseded while in flight can finish with
App authority. Its reply can then start downstream work that the same reply
would be refused permission to start while the original resource still owns
it. This bypasses the runner's `person-required` restriction for an app's
declared child-process capability; it does not grant undeclared programs.

`ModuleWork` retains only the runner ticket and source token for kept work
(`runner/src/runner/module_work.rs:20-21`, `172-176`). The origin tables are
pruned to ordinary pending requests (`runner/src/runner/modules/admit.rs:571-576`).
`fulfill_now` takes the module-work branch before restoring the request's
origin, invocation and causes (`runner/src/runner/commit.rs:1170-1172`,
`1220-1223`). `module_work_landed` calls the source and commits store changes
under the current origin, ordinarily App (`module_work.rs:188-210`).

Reproduced with a small plan and DataSource against this checkout's existing
runner, plan and kernel libraries:

1. A Tool action starts a POST. Its successful mapper writes a declared store key.
2. A second resource reads that key and asks for a declared process when the key appears.
3. As a control, fulfill the POST normally: the process is refused as `person-required`.
4. In a fresh runner, supersede the POST's resource before fulfilling it. The source retains the sent request through `keep_sent`.
5. Fulfill the retained POST. The same downstream process request is emitted with `reach.origin = App`, with no admission refusal.

The local probe `/tmp/exact-runner-module-origin-probe.rs` asserts both paths
and prints the original Tool origin and the resulting App origin. It only
collects requests: no network request or child process is executed. This
demonstrates the runner's admission error, not a complete application attack.

Keep origin, invocation and accumulated causes with retained module work.
Run its continuation and the resulting commit under those facts, as the
ordinary reply path does. Cover the normal and superseded cases with the same
restricted downstream request, including a reply whose store write causes a
resource to be asked again.

No matching ticket found in this checkout or the open and closed issue
corpora in the local `exact2` and `exact` checkouts.

## Resolution

Kept requests retain their origin, invocation and causes before the original ticket is retired. Their continuation and resulting commit run under that retained authority.

## Verification

`kept_tool_reply_retains_its_authority` passed. Both ordinary and superseded tool replies are refused with person-required when their store writes trigger a process request.
