# Lanterns change trials — third interleaved round

Work in progress: retained rows below are historical evidence; new dispatches are
recorded individually as they finish. No new outcome is inferred before evaluation.

After2 is pinned to `a30e5e36b3be8d518471b76b1b439af5e227f98a`, this checkout's
initial HEAD, containing the engine re-merge, EXCAP v2 and E1. The local
`next/trunk` ref instead points to `7de897543937341668ef122b0d4fe98d02be3ef7`,
which predates those requested changes. With no clarification available, the
specified feature boundary wins; neither ref was moved. This is an explicit
ref-selection deviation from a literal `next/trunk` lookup. Baseline remains
`e47a09fc36c07103cdc3645b7061cca305af6c14`; prior after remains
`7c65726ca8be72a3f10de3a44a836e1b3b00eed5`.

[Protocol and frozen hashes](evidence/I4/protocol.json). Evaluator v3 and prompts
are unchanged. Fresh CLI sessions use gpt-5.6-sol, high reasoning, a 900-second
ceiling and the frozen three-repair rule. Dispatch order: after2 A1, baseline A4,
after2 B1, baseline B4, after2 A2, after2 B2. No checkout builds run concurrently
with trials. Each trial's sources/auth/build outputs are deleted after evaluation.

Times are wall seconds. Tokens are uncached input / output. Historical rows and
unknown cells are preserved from [the prior report](RESULTS-after.md), including
original A1's infrastructure abort and the browser references' limited metadata.
Build/evaluation exclude preparation and model time; invocation and repair counts
retain the prior report's definitions. Functional task success is distinct from
strict full acceptance, which also requires common-route and world-pixel checks.

| Cohort | Attempt | Agent s | Build s | Eval s | Outcome | Uncached input / output tokens | Files | + / − text lines | Build / proof / test calls | Repairs (largest loop) |
|---|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| Baseline original | [A1](evidence/next-baseline/a-1/trial.json) | 620.1 | 17.7 | 18.0 | Infrastructure abort; no edits | unknown | 0 | 0 / 0 | 0 / 0 / 0 | No model work |
| Baseline original | [A2](evidence/next-baseline/a-2/trial.json) | 516.2 | 17.6 | 20.5 | Task pass; instrument/context deviations | 83,805 / 15,178 | 7 | 158 / 12 | 1 / 1 / 3 | 1 (1) |
| Baseline original | [B1](evidence/next-baseline/b-1/trial.json) | 268.1 | 18.5 | 19.8 | Task pass | 54,432 / 9,254 | 5 | 51 / 16 | 0 / 1 / 4 | 3 (3) |
| Baseline original | [B2](evidence/next-baseline/b-2/trial.json) | 494.9 | 17.6 | 17.9 | Fail: phase contract | 122,573 / 17,719 | 5 | 83 / 19 | 0 / 2 / 9 | 3 (2) |
| Baseline fresh | [A3](evidence/I2/baseline/a-3/trial.json) | 313.9 | 17.5 | 20.6 | Task pass | 67,853 / 11,414 | 7 | 160 / 12 | 1 / 1 / 5 | 3 (2) |
| Baseline fresh | [B3](evidence/I2/baseline/b-3/trial.json) | 216.4 | 17.6 | 17.7 | Fail: phase contract | 52,451 / 8,549 | 5 | 60 / 18 | 0 / 2 / 1 | 1 (1) |
| Baseline fresh | [A4](evidence/I4/baseline/a-4/trial.json) | 308.0 | 17.4 | 20.5 | Task pass | 91,475 / 11,936 | 7 | 152 / 29 | 1 / 1 / 3 | 1 (1) |
| After | [A1](evidence/I2/after/a-1/trial.json) | 541.6 | 20.0 | 21.3 | Task pass | 178,032 / 18,170 | 13 | 906 / 671 | 1 / 2 / 12 | 8 (3) |
| After | [B1](evidence/I2/after/b-1/trial.json) | 259.9 | 18.3 | 18.7 | Fail: phase contract | 63,217 / 9,696 | 5 | 65 / 15 | 0 / 1 / 4 | 3 (2) |
| After | [A2](evidence/I2/after/a-2/trial.json) | 483.3 | 18.4 | 21.5 | Task pass; context deviation | 154,867 / 18,857 | 13 | 815 / 696 | 0 / 3 / 6 | 4 (1) |
| After | [B2](evidence/I2/after/b-2/trial.json) | 400.0 | 18.2 | 18.7 | Fail: phase contract | 80,958 / 14,329 | 5 | 103 / 27 | 0 / 3 / 5 | 3 (2) |
| After2 | [A1](evidence/I4/after2/a-1/trial.json) | 506.0 | 20.2 | 21.1 | Task pass | 310,295 / 12,631 | 8 | 130 / 13 | 0 / 2 / 2 | 1 (1) |
| babylon reference | A1 | 407.2 | 2.0 | 20.9 | Pass | 63,450 / 13,307 | unknown | unknown | unknown | unknown |
| babylon reference | A2 | 468.7 | 2.1 | 20.4 | Pass | 51,682 / 12,038 | unknown | unknown | unknown | unknown |
| babylon reference | B1 | 220.4 | 2.2 | 20.2 | Pass | 49,007 / 8,330 | unknown | unknown | unknown | unknown |
| babylon reference | B2 | 347.3 | 2.1 | 19.9 | Pass | 53,998 / 13,441 | unknown | unknown | unknown | unknown |
| playcanvas reference | A1 | 477.5 | 0.5 | 8.3 | Pass | 69,290 / 20,628 | unknown | unknown | unknown | unknown |
| playcanvas reference | A2 | 339.4 | 0.5 | 8.5 | Pass | 53,618 / 13,981 | unknown | unknown | unknown | unknown |
| playcanvas reference | B1 | 457.5 | 0.5 | 8.2 | Functional pass; repair-cap failure | 55,906 / 18,233 | unknown | unknown | unknown | 6 (5); cap exceeded |
| playcanvas reference | B2 | 566.2 | 0.5 | 9.2 | Pass | 53,345 / 15,291 | unknown | unknown | unknown | unknown |

