# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 4 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `bf54bb3be6`.
- **Method:** one brief (sha256 `4a7cb78121a952c7cb093b936323559ec3e09596eac47ec8a1b11851a1c91d88`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- `host/terminal/src/requests.rs:66`. `run_requests` does call `long_poll_refusal` before `dispatch_work` (`:71`) and passes `is_ordered()` into `refuse_request`. That is the whole terminal path. Nothing in this host calls `take_request_refusal` or `has_ordered_request_refusals` (`runner/src/runner/admission.rs:187` says the host must keep later ordered admissions behind that flag). `land_requests` only fulfills channel outcomes, so the source never receives `Refused`, the ticket stays pending, and a background round stays armed (`background.ticket` until `background_landed`). The same loop still `dispatch_work`s and runs the next ordered continuation, and the next commit does too. `cargo test -p exact-terminal` is green and does not exercise this branch.

**Should-fix**

- `host/render/src/lib.rs:909`. A long-poll admission error still lands in `held.busy` (`:854` sends `Dispatch::Missing` into `run`). The render settles `Busy`, which is HTTP 503 (`:88`). The executor does refuse before work and does fence; surface and storage refusals use `held.refused`.

**Nits:** None.

Page enumeration holds. `host/render/src/pages.rs:65` forces `Dispatch::Missing` before `data.dispatch`, then `executor.run(out, None)`. Admission refuses, so `again` (`:73`) and the continuation never run. The fence test `a_long_poll_on_other_work_is_refused_at_admission_and_fences_ordered_work` passed: native work is independent and the following GET is recorded; storage and continuation are ordered and that GET stays empty. `cargo test -p exact-apple --lib long_poll`: 13 passed, 2 ignored.

Other host takes of runner requests refuse before work: `host/apple/src/abi.rs:263`, `host/linux/src/presenter.rs:769` (both through admit, then the pump settles), `host/web/src/host.rs:1002` (`request_refusal` before `dispatch_work`; the page fulfills kind 2). `Host::take_requests` on Apple and Linux only forwards. Presenter and canvas `take_requests` are not runner requests. Data seams refuse before any rewrite or send: `data/host/src/lib.rs:89`, `data/src/envelope.rs:269`, `js/src/wire.rs:96`, `logic/abi/src/lib.rs:285` and `:530` (decode fails before `call_reply` applies writes). `background()` only forwards the request; work starts when a host dispatches it.
