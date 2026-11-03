# Refreshing one permission waiter strands the others

**Status:** Closed
**Severity:** P2
**Systems:** `runner/src/runner/modules.rs`, `runner/src/runner/modules/keyed.rs`
**Author:** Codex
**Date:** 2026-11-02
**Commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

Under the agent's permission substitute, refreshing a resource while its permission prompt is pending removes that prompt and all calls sharing it. Another already dispatched resource remains pending indefinitely, with no ticket the agent can answer and no work to release. The replacement request dispatches as `Missing`.

`shared_hold` deliberately gives calls waiting on one permission the same device ticket (`runner/src/runner/modules/keyed.rs:149-163`). Cleanup finds a superseded call by continuation token but then removes every held call with its device ticket (`runner/src/runner/modules.rs:1382-1389`). That includes calls still in `pending`, and a replacement call that just joined the existing prompt.

Reproduced on this Mac in `/tmp/exact-runner-shared-permission-probe.rs`, linked against this checkout's warm Rust rlibs. Its plan has two string resources reading one module source, declared `request="when-prompt"`, with a `location` permission initially prompting. Both requests dispatch as `Held` and share one device hold. A `refresh first` action then reports:

```text
Before refresh: two requests share one permission ticket
After refresh: device holds=0, pending=[("second", 3), ("first", 4)], new requests=1
Remaining dispatched request has no permission ticket or released work; replacement dispatch is Missing.
```

The probe checks that `second`'s runner ticket remains held, its released-work list is empty, and dispatching the replacement continuation returns `Dispatch::Missing`. It uses no platform prompt or network. This establishes the runner/substitute failure; production permission prompts answered wholly by a native implementation were not exercised.

Remove a superseded call by its own token. Retire a shared device ticket only once no surviving held call uses it. Cover refresh, argument supersession and cancellation while two calls share a permission prompt.

Deduplication: searched current `issues/` and open and closed issues in the local `exact2` and `exact` checkouts for shared permission tickets, retirement, supersession and stuck permission requests. No matching report found.

## Resolution

Retirement removes the obsolete held-call token and retires its shared permission hold only after the last waiter leaves.

## Verification

`refreshing_one_shared_permission_waiter_keeps_the_others` passed: refreshing one waiter preserves the shared hold, and answering it releases both the surviving and replacement calls.
