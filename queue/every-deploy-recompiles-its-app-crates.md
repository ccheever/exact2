Every deploy recompiles its app crates (2026-09-23, deploy lane): their build
scripts write the run's receipts into a per-run `EXACT_BAKE_OUTPUT` they watch, so
an unchanged deploy still reruns them and recompiles `caltrain-web`, both
`caltrain-apple`s and `caltrain-linux` with LTO; nothing else rebuilds, and a warm
Caltrain deploy took 265–450 s at load ~25. A receipt directory per Cargo cache,
copied into the run after the build, would let an unchanged deploy reuse them. The
`exact-filesystem` helper also rebuilds inside each deploy's capture, where its tool
target sits.
