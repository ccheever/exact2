# Code review: a kept answer's identity read from a fallback (LLP 1027.005 D3) (63db2ec5e), 2026-10-10 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `42309550a8cf33cd1b93cc86552a0f970c71fd02d91599d0494476adecc84f28`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Both MAJORs taken in 75eae7437: only the identifying arguments are walked, never the `with` context; unknown means a store-reading resource on its compiled answer or placeholder, so kept, carried and checkpoint answers and non-store values stay known. The journal's shortened identity ends in an ellipsis.

---

The dependency walk and the settlement gate are in place, and the signed-out placeholder collision is closed. Two of the checks you named still refuse a legitimate seed.

## Findings

[MAJOR] A carried or checkpoint answer is treated as an unknown identity — `runner/src/runner/kept.rs:160`

`identity_unknown` reports a dependency unknown when `self.stale[r]` is set and that resource is not itself a kept seed. On an unready boot every store-reading resource is marked stale before settlement, including one that already holds a value (`runner/src/runner.rs:950`). A carried answer and a checkpoint answer taken while the device has app store entries are both in that set: `kept_seed` is false, `placeholder` is false, and the value is not compiled. `carry` only omits stale or pending resources, so a reload can carry a fresh `account` and then re-seed `feeds` from the store because `feeds` was still in flight. The first frame drops that kept `feeds` even though its call arguments were computed from the account value this launch is showing. The same split happens for a document checkpoint: answers that were fresh are shown and marked stale, and a kept seed for a resource the checkpoint omitted is refused. The amendment admits a seed unless a call argument reads a fallback (compiled answer or placeholder), and D3 says carried and checkpoint values keep their own rules. `stale && !kept_seed` is a wider proxy. A non-store resource, a runner-owned fact, a compiled else-row that is not stale, and a source that is ready at boot are not refused: those are not stale, or `unknown` is gated on `!data.ready()`.

[MAJOR] The walk includes `with` context, so a known call identity is refused — `runner/src/runner/kept.rs:155`

The walk starts at `deps.resource_args[i]`, and that set is every argument expression (`runner/src/instance/deps.rs:207`). Trailing `with` arguments are in that union; the `context` count is never applied. A context expression that loads a deferred resource, or a derive that loads one, makes `identity_unknown` true even when every call argument is a slot or a kept seed. `status(car) with freshness` then drops yesterday's status of this car whenever `freshness` is still on its placeholder. The new comment and the amendment both say call arguments, and D3 exists so context does not decide admission. Slot context, which is the `with minute` case, is ignored and still admits.

[NIT] The journal cap cuts the identity with no marker — `runner/src/runner/lines.rs:66`

`chars().take(200)` keeps a prefix of the JSON and does not say the arguments were shortened. A long key is no longer the key the line exists to record. Request lines log the full URL. The line is otherwise only resources that still have `kept_seed` after settlement, which are the seeds the first frame retains, and the arguments are the identifying list, not the answer.

## Assessment

The direct and one-derive placeholder collision is closed, and a kept account still admits its dependent. Derive cycles stop on a visited set. The fixpoint evaluates a resource or derive only after the inputs it loads have settled, so a dependency decided later in the pass is already updated in the tentative `states` before the reader is gated. Pending and failed opcodes also set the resource bit, so a status read of a fallback resource counts as a read. Frame and row-slot reads are not a second path: those bodies run with empty frames and trap rather than aliasing a fallback. Opaque flag bits do not hide a later `LoadResource` unless the instruction stream itself is malformed.

`unknown` also blocks the failed-argument and in-flight reuse clauses, but only while `kept_seed` is still set. Once that commit stands, the mark is cleared and the value is the fallback, and nothing reads the store entry again before the next boot, so a seed refused once does not return. A settle that returns an error drops the tentative `states` clone. A gate refusal restores the checkpointed resources, so a seed cleared only inside a refused commit comes back. After readiness the gate is off, and an admitted seed still stands for an asynchronous answer the way D4 describes.

The new tests cover the placeholder, a derive of it, a kept identity, and the journal line for that launch. They do not cover a `with` tail, a carried answer, or a checkpoint answer.

NOT READY
