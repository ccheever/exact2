# Code review: the design tool's half of LLP 1121 stage 11 D on the real server (the stub dropped), round 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7686803747`.
- **Method:** one brief (sha256 `7255fd3a5a7b8f40c05aaca12af468457e43bc769e0f12f2a7999fb539ed0177`), shared by both reviewers, each blind to the other's review; commits 60bf87b1d8 and 7686803747, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 3 NIT).
- **Disposition:** taken post-round, in the commit named in the sign-off request:
  - radius.full's proposal is checked active with its diff, then applied (1).
  - `bases` are inputs (2).
  - A cell's explain waits as long as a read (5).
- **Not taken:** neutral's place among the shipped columns, beyond describe's order (3); the package value as a literal (4). Both come from the fixture's own design.

1. **SHOULD-FIX** — `radius.full`'s "the server takes it" assertion passes when the proposal has not answered. `scripts/design-tool-scale.test.mjs:121-123`

   The drive is `tap set-for-app-radius.full`, `clock data`, `clock +1`, `clock data`, `state`. `clock +1` runs `after(1, advance)` (`apps/design-tool/app.contract:849-850`), which writes `filesAt` and starts propose again. `clock data` gives up at 20s and the drive continues (`host/web-js/agent.js:354-357`, `scripts/agent.mjs:1207`). Propose itself is allowed 130s (`apps/design-tool/app.ts:37-40`, `:783`). Until that answer lands, the resource is still `else empty(ok=true)` (`apps/design-tool/app.contract:769`) or the previous remove-token proposal, and both have `ok: true`. `toMatchObject({ ok: true })` matches either. The slot check on the same line only shows that Set put `9999px` in local state. A fast refusal (`ok: false`) still fails the test. A proposal still in flight does not. Set, Back, and both undos stay exact: `writes[n].after` is the file read after the server's reply (`scripts/design-tool-harness.mjs:95-97`), compared whole at `scripts/design-tool-scale.test.mjs:95`, `:100`, `:103`, `:108`, `:111`. Assert `proposal.active` and `proposal.key` for this draft, or Apply and compare the file, the way `space.md` already does.

2. **SHOULD-FIX** — The recorded skip of `explain.bases` does not match the server. `apps/design-tool/app.ts:748`, `:757`

   The round-1 record says `names` already lists every token a value reaches and `bases` is only that list's base-name subset (`llp/reviews/code-2026-10-20-dtool-s11f-r1.astra.md:16`). Unset base names are stored on `Checked.bases`, not `Checked.names`: `names` is design-token refs (`tamber/src/check.rs:202-208`); a bare `space.md` is recorded only when the fold is a named length (`tamber/src/check.rs:1112-1114`). `explain` sends those on `bases` (`contract/cli/src/tamber_base.rs:82-90`, `:320-324`) and leaves them out of `names` (`:347`). `loadBranch` types the reply without `bases` and passes `raw.names` to `inputsOf` (`apps/design-tool/app.ts:741`, `:748`, `:757`). Scenario: `card.pad: length = space.md` with `space.md` unset. The branch value is the text `space.md`, `expression` stays false, and there is no "Follows space.md" link (`apps/design-tool/app.contract:1788-1791`). `full` and reserved `stretch` are right: `{kind:"full"}` is "the pill" (`apps/design-tool/base.ts:26`), and `9999px` is the kernel's pill length (`kernel/src/style/scale.rs:28`, `:68`). `ScaleSource` has no stretch arm (`kernel/build/scale.rs:201-207`); the Mac's space.xl is a metric on the wire. The tool's stretch arm can stay for when the table emits it.

3. **NIT** — Copying the web crate does not lock the shipped neutral column. `scripts/design-tool-scale.test.mjs:26-28`, `:77`

   `shipped_targets` treats `web/Cargo.toml` as a shipped web target (`contract/lower/src/icons.rs:172-176`), then sorts by name, so this app's columns are `ios`, `mac`, `neutral`. That is also the table's own order (`apps/design-tool/base.ts:13`). `order(first)` equals `shipped` either way. Delete the `web/Cargo.toml` copy and the test still passes: neutral drops back behind "Other platforms", which is the round-1 nit, and the order check still agrees with `describe`. Assert `shipped` contains `neutral` on the unexpanded row.

4. **NIT** — The package Set value is still the fixture literal. `scripts/design-tool-scale.test.mjs:106-108`

   The package name now comes from `described.ir.tokens` (`:48-49`). The draft and the written line are still hardcoded `640px` and `: length`. A tool that wrote those bytes without reading the package row would pass. Read the value from `resolve`, the way `space.md` reads its column from `describe`.

5. **NIT** — The two new real-server cell opens still explain with the 15s default. `apps/design-tool/app.ts:739`

   `loadBranch`'s `explain` does not pass `READ_WAIT`. The package cell and the `radius.md` override are the read-only and editable checks (`scripts/design-tool-scale.test.mjs:115-120`). An explain queued behind the events poll fails those asserts at 15s while propose is allowed 130s. Same fail-closed shape as the other real-server drives; this test is the one that now depends on it.
