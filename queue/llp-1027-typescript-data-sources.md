LLP 1027 (TypeScript data sources) is **Accepted** (Charlie, 2026-09-03, every
recommendation; implementer Claude, stage 1 landed the same day: the `sources` table in
the plan (format 3), `DataSource::bind`, the `exact-js` crate over the lean Hermes VM
with Caltrain's TypeScript twin as its byte-equality fixture, Rolldown at the repo root).
Stage 3 landed the same day: `fetch` over the host's ticket path (a prelude in
bytecode; one host door with four ops for requests and the store), `parse` returning
an `Answer` so one answer awaits two fetches in a row, eleven tests. Next: stage 2
(bake: Rolldown → hermesc → bake under the VM, `app.d.ts`, `dev.mjs`) built against
its first consumer, stage 5 (Weird Castle in TypeScript), then stage 4 (web), stage 6
(delivery), stage 7 (the engine crate + Linux). LLP 1027.001 supplies the named
Ibex URL, UTF-8 text and base64 bindings. LLP 1027.000 landed 2026-09-04: time and seeds
are explicit inputs; ambient Date/Intl time and Math.random calls refuse. The
boot question was ruled the same day ("ok let's do that"): **the kept answer** — the
runner persists a store-reading resource's last answer beside the secrets, boots
from it when the source is not ready, falls back to the bake's empty-store placeholder
(plan format 4, `resources.reader`), and `data_ready` asks again once the engine is up;
landed with a test through the runner. Written the day Charlie
ruled "TS support optional, but the default paved path for anything with substantial
app logic/business logic." Nothing above the data seam changes: `app.ts` exports
`appId`/`grants`/`answer`/`parse`, values cross as JSON directed by the plan's own
`fields` table, the module has no globals that reach out (a request is a value the
host runs), bake compiles it with `hermesc` to bytecode for native and to JS for the
web, and a new crate `exact-js` (the lean Hermes VM, ~100 lines of C++) runs it after
first pixel; Rust stays behind the same seam for hot sources (D8). Measured: Caltrain
ported to TS answers 20/20 cases byte-identical, 0.32 ms create + 0.008 ms load,
1.2–41 µs a call (7.5–29× native, mostly JSON), the linked lean engine +1.81 MB
stripped (+785 KB gz), TS→bytecode 20 ms. D9 answers "split ibex2 in two": it is three
— `ibex2::host` (linked today), the engine build as a `-sys` crate (proposed), ibex2's
JS runtime layer (never linked). Waits on §8's trades (D4's sentence, "HBC
compilation" for the bake, 1026's wasm data module leaves its staging so a phone
carries one interpreter) and §10's eight questions — 1.8 MB on iOS and the
one-frame `pending` first. 1019's link (Accepted, landed) left `current/` for it.

*Filed under “Later”.*
