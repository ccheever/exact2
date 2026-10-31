# Pending cases

Reduced programs whose two evaluators are known to disagree, kept here so the
disagreement survives a cleared `target/` while `QUEUE.md` holds its entry. The
corpus collector walks `semantics/corpus/` only, so these never run in `difftest
corpus` or `quick`; run one by hand:

```
cargo run -p contract-difftest -- corpus semantics/pending/<name>.contract
```

When a case agrees, move it into `semantics/corpus/` beside its kind and delete its
queue entry.
