# EX10 latest upstream integration review — codex

Candidate: `9b786cd57e9f59be32c80f4295cdfc633ffd9c81`. Public baseline: `76726a12f6c25b2241f12779991ea6a3f5b0bb2d`.

Provenance: fresh codex exec, gpt-6.1-sol, high reasoning effort. Bounded read-only review of the latest disjoint upstream integration; no other-family response or author reasoning supplied. Launcher envelopes omitted. Received response follows verbatim.

READY — no material integration issues found in `9b786cd57`.

- EX10’s migrator, app-lock handling, helper claims, origin diagnostics, refusal assertions, and scaffold rollback are unchanged from `866743ca5` through the latest merge.
- Every path changed by upstream `511bdf55d..76726a12f` matches upstream exactly in the candidate. The design-tool and Android/material-oracle changes introduce no material interaction with the reviewed EX10 behavior.
- `Cargo.toml` adds only `apps/material-specimens/linux`; `Cargo.lock` adds its package entry without changing existing resolutions. Lock enforcement and helper capture remain intact.
- The approved public `0.4.29` fallback and `snapback4/readme-tested.json` are unchanged from `86640cf75`. `node scripts/check-snapback-readme.mjs` passed, validating five example receipts and both dated claims against the retained pin.

Gate reruns were neither repeated nor observed; their results remain unconfirmed. No source changes or commits were made.