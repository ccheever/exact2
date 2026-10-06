**Contract modules, after stage 2** ([LLP 1091](../llp/1091-contract-modules.rfc.md), Charlie
2026-10-04): both stages landed and nine code-review rounds are disposed (§10–§18). Open: a
Rust build does not see a Contract package installed nearer than the one it resolved
(Cargo has no non-recursive directory watch; §18). Outside this repo, Lexy can drop its own
`timeline Activity` for `use Activity from "exact:motion"`.

*Filed under “Next, in order (2026-08-29)”.*
