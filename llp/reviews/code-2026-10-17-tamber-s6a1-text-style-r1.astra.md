# Code review: A text style carries its weight and leading per platform (LLP 1115.000 D12.2, Tamber stage 6 A1), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD, the change.
- **Method:** one brief (sha256 `759d1bc8be06c69f6cd272d99fec86bf2de4a0395c00b5df0574c6c3f9497317`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (P1s: the live web and the document lost a shorthand's rows, a bound style moved only the size on the JS target, a derived change left descendants measured; P2s on control starts, the button title, a child's size and a clear).
- **Disposition:** in the first-round commit.

---

Blocking issues remain.

- **P1 — Live web and SSR lose shorthand weight/leading.** [shorthands.rs:34](/tmp/rv-s6a1-1/contract/lower/src/shorthands.rs:34) removes the rows, but only the JS template supplies replacements. Under `font-weight=700 line-height=2`, `font="-exact-body"` still renders as 700/2 through the live web/document writers, rather than 400/normal.

- **P1 — Bound JS styles update only size.** [style.rs:692](/tmp/rv-s6a1-1/host/web-js/src/style.rs:692): `font=(bold ? "-exact-headline" : "-exact-body")` has no static style for the template to resolve. Its runtime writes only `font-size`, leaving headline at weight 400 instead of 600.

- **P1 — Derived changes do not invalidate descendant measurements.** [txn.rs:786](/tmp/rv-s6a1-1/kernel/src/txn.rs:786): switching a parent’s shorthand from body to headline propagates `FontSize`, which stops at a child with `font-size=12`; that child’s inherited weight changes without invalidating its paragraph metrics. Changing [FontShorthand:1572](/tmp/rv-s6a1-1/kernel/tables/schema.json:1572) likewise lacks metric invalidation.

- **P2 — JS bare-style defaults override dynamic inheritance.** [template.rs:65](/tmp/rv-s6a1-1/host/web/src/template.rs:65): beneath `font-weight=(heavy ? 700 : 300)`, a static `font-size="-exact-headline"` child gets baked `font-weight:600`. It consequently inherits neither value. Component-bound ancestor rows have the same problem.

- **P2 — Control starts outrank the shorthand.** [defaults.rs:158](/tmp/rv-s6a1-1/kernel/src/arena/defaults.rs:158) visits control defaults before text-style defaults. A native input naming `font="-exact-headline"` retains its control weight/leading. Meanwhile, `text_style_source` reports the shorthand as the source, disagreeing with the value resolver.

- **P2 — Native button export drops the derived weight.** [button.rs:197](/tmp/rv-s6a1-1/kernel/src/arena/button.rs:197) restricts the computed title mask to stored rows. For a button containing `text font="-exact-headline"`, this removes the newly derived weight before serialization; Swift and Linux button presenters fall back to their ordinary control weight.

- **P2 — A child’s size incorrectly cancels inherited bare-style weight.** [text_style.rs:72](/tmp/rv-s6a1-1/kernel/src/arena/text_style.rs:72): beneath bare `font-size="-exact-headline"`, a child with `font-size=12` computes weight 400. The emitted JS CSS inherits 600, so kernel measurement and browser rendering disagree.

- **P2 — Bare-style fallback ignores authored clears.** [defaults.rs:236](/tmp/rv-s6a1-1/kernel/src/arena/defaults.rs:236): `font-size="-exact-headline" font-weight="inherit"` with no ancestor weight computes 600 instead of 400. The fallback does not check `written`; measurement and provenance repeat the error.

Four prebuilt focused tests passed, but omit these cases. No worktree or index changes.
