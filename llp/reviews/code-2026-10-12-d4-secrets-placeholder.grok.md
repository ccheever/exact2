# Code review: a store reader's placeholder at a launch holding secrets (LLP 1027 D4) (26c76fe56), 2026-10-12 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `4e8b4caa9f2d44c8671889c46d74089152c370ce9cb58b569b6e47ca0d8e6639`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Astra's two MINORs (a secret written or forgotten before readiness flipping the choice and its flags) are taken by deciding on the launch's snapshot (`Store::opened_with_secrets`); Grok's two NITs (comments) are taken, and a test that never saw its entry is replaced.

---

Review of `26c76fe` (LLP 1027 D4 amended). One comment defect. The settlement guard matches the amendment.

## Findings

### NIT — `runner/src/runner/settlement.rs:96`

`secrets_unseen` was inserted under `is_placeholder_row`'s doc comment. The comment now opens with "Whether resource `i` is another's `else` row", which this function does not answer, and `is_placeholder_row` at line 110 has no comment. Move that sentence back, and leave `secrets_unseen` a single line on why the bake's empty-store value is passed over.

### NIT — `runner/src/runner/settlement.rs:542`

The next arm's comment still says it runs when nothing is compiled. That arm is now also the secrets pass-over, where a compiled value exists and was skipped. The behavior is right; the comment is the part a later edit could "fix" by requiring `initial.len == 0` and undoing D4.

## What was checked

The guard is only on the compiled-value arm, and only while the source is not ready, the resource is a store reader, a declared `else` can be shown, and `holds_secrets()` is true. An admitted kept, carried, or checkpoint answer is reused before that arm. A refused kept answer (wrong identifying arguments, or an identity still unknown) misses reuse and takes the `else`.

`else empty(…)` is `placeholder_value`. `else source()` is waited on first (`settlement.rs:392`), so the placeholder row has already settled when the guard reads it. If that row is itself a store reader with no `else`, it keeps its own build-time value and the outer resource shows that value, pending. `data_ready` forces every stale resource; with the source ready, both this guard and the compiled arm are off, so the source is asked. The new test reaches `signed in as alice` only if that ask runs. Revalidation stays quiet on a placeholder, which is right: that frame did not show the bake. Stale compiled resources still get the build-time line, in plan order, once, at boot.

A placeholder makes a dependent's identity unknown. That is the existing LLP 1027.005 rule: `identity_unknown` already treats a compiled fallback and a placeholder the same, and admission of a kept seed also requires the source not to be ready, so the refusal does not stick after `data_ready`. An admitted kept answer is still a known identity.

A ready source, a non-store resource, and a store with no granted secret stay on the build-time answer. Later passes reuse that placeholder while the arguments and the store revision hold. `set` bumps the revision, so a secret written before readiness is picked up on the next pass. A refused pass does not publish; `conclude` restores `stale` with the store.

`holds_secrets` is the right test for this rule: a granted name present in the snapshot. `exact.kept.*` and `exact.state.*` are runner-owned, so they are not granted. Apple's Keychain values are in that snapshot. Files and SQLite are not store entries, so a storage reader still shows its kept answer or the bake unless some other granted secret is held. The amendment is about a store that holds a secret.

The journal line is the resource name plus a fixed sentence, on the agent `logs` journal, with no secret name or value. It is recorded once at boot, and only for a store reader that actually took the placeholder.

The tests pin secret plus `else`, empty store, kept-only entries, a refused kept answer, an admitted kept answer, no `else`, and then `data_ready`. `an_ungranted_entry_is_not_a_secret` never sees `other`: `Store::from_names` drops ungranted names before the check. The kept-only case is the one that pins runner-owned entries. `else source()`, carry, and checkpoint are not booted in these tests; those paths are the reuse and placeholder-first waits above.

`rt.js` still shows the bake. The LLP and `QUEUE.md` say so.

Full notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-4063e000.md`

READY
