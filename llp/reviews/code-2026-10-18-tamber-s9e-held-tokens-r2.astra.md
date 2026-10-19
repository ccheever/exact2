# Review round 2, LLP 1121 stage 9 E (held rows name their tokens), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING — [contract/cli/src/tamber_serve.rs:72](/tmp/rv-s9e-r2/contract/cli/src/tamber_serve.rs:72): The protocol remains `tamber/1.11`.**  
   `hello` returns this constant, despite the ruling that this commit opens 1.12. Update it to `tamber/1.12`, update the hello tests, and correct the protocol guide’s introductory version references.

2. **BLOCKING — [host/web/input-glue.js:407](/tmp/rv-s9e-r2/host/web/input-glue.js:407): Known role overrides lose their token names.**  
   A development build maps `--exact-canvas-text` to `CanvasText`, but `modes.token` recognizes only `--x-*`. Holding a row using `var(--exact-canvas-text)` therefore returns neither `token` nor incomplete provenance. I reproduced this for active and hover rows. Resolve custom properties present in `modes.js`’s `Names` table, including role overrides, while continuing to exclude ordinary host properties. Add a regression using the real token resolver.

3. **SHOULD-FIX — [contract/cli/tests/it/state_looks.rs:191](/tmp/rv-s9e-r2/contract/cli/tests/it/state_looks.rs:191), [host/linux/src/presenter/held_tests.rs:155](/tmp/rv-s9e-r2/host/linux/src/presenter/held_tests.rs:155): Required formatting check fails.**  
   `cargo fmt --all -- --check` exits 1 on the added tests. Format these files and rerun the check.

The other round 1 fixes look correct. Caps and seven focused JS tests passed; in-memory checks confirmed owner identity, differing instance paths, imported-source freshness, and authored-versus-direct D7 naming. Rust/native integration tests were not run under the read-only sandbox.
