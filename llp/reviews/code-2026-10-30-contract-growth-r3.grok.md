# Code review: the Contract compiler's quadratic growth, round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3d538342ed`.
- **Method:** one brief (sha256 `b38fc57019ef18cdf5f0e3730e1b69dd12f133e949abc5b0d7bd8c09ddb161d8`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the three commits against the approver's conditions, starting with the diff and the round-2 fixes, without changing the checkout.The tip is three commits past `35e89d4068`. I'll read the full diff and the surrounding lookup, retype, and growth-test code.The three commits match the approver's conditions. The name maps and the narrower re-typing keep the old answers, and the new tests fail if an index or a refresh drifts.

No findings.

What holds:

- Lookups stay innermost-first, and the first entry in a frame still wins. `Frame::find` builds a first-position map only for that frame, and `push_name` does not replace a name already in it. `an_inner_frame_shadows_an_indexed_outer_one` uses two indexed frames and expects the inner `Param`. `an_indexed_frame_finds_a_names_first_entry` expects the earlier duplicate. `a_parameter_shadows_a_state_in_a_large_component` expects the parameter, not the outer state `n`.
- Re-typing matches a fresh scope for the cases that can change a type. A body writes `ct.slots` only in `actions.rs` on a successful assign, and `retype_written` refreshes those indices. Under test, `all_slots_retyped` compares that scope with every slot refreshed. `calls::refine` keeps one declarations scope, drops the caller's clone before `retype`, and `same_declarations` compares frame 0 with `component_scope` before the next caller. Actions are a suffix of that frame, so `first_action + ci` is the callee. The 300-component property test passed, including settled options and refined parameters.
- Lowering and the warning maps are lookup-only and keep first-wins. A state still overwrites a mutation of the same name in `slot_names`. Sends still use `mutation_names` only. The button-face walk reads the same `(span, instance, tag)` range the old filter did, and caches a span's answer only when every instance agrees.
- `growth.rs` is `#![cfg(unix)]`, which matches `libc` under `[target.'cfg(unix)'.dependencies]`. The comment records 17.1x on main. `contract/lower/src/lib.rs` and `contract/types/src/lib.rs` are 1,487 lines by the caps count (`split('\n')`), under 1,500. The Swift file over the cap is unchanged on this base.

I ran `cargo test -p contract-types --lib` (10 passed, including the property test and the shadowing tests) and `cargo check -p contract-lower -p contract-analyze -p contract --tests`. I did not re-run the full contract suite, clippy, or the 11,100-plan byte compare. `rustfmt --check` was blocked by the sandbox.

Non-blocking notes:

- The growth fixture still has no send, mutation `then`, module event, view event, or launch route, so a per-use scan put back only on those paths stays under 4.6. The 4.6 line sits in the measured gap between a clean 3.9–4.3 and the weakest single-scan regression at 4.8–5.2.
- `calls::check` still uses `c.actions.iter().find` per call (`contract/types/src/calls.rs:220`). That walk was already there. At 250 versus 1,000 uses it stays inside the linear band, so the ratio test does not pin it.
- The `#[cfg(test)]` checks rebuild a full declarations scope per calling action and retype every slot per body. They are off when the CLI growth test links the library, which is why that ratio still measures the production path.

APPROVE
