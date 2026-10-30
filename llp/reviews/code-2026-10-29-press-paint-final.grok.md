# Code review: web press paint keeps authored transitions, final narrow re-check (capped) (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b9c8345cfb`.
- **Method:** one brief (sha256 `4ed4596e5802497bc6a902be8355b17a7e937b2b25de74be6c47f55203278ab3`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## Findings

**MAJOR** `host/web/input-glue.js:392` (used at `:354`). `keptBase` is meant to be the look beneath the kept declaration was written from. A re-read that keeps the declaration (`:349` applies it, then `:350` reads) stores the painted value instead: opacity from `computedStyleMap` / `cs.opacity`, color from `cs.color`. After a turn or a settle, both are the pressed look (0.4, red), not the base (0.8, black). Dropping the declaration then compares the new transition's target to that painted value. A glue-started transition back to the real base does not match, so it is left running (Firefox). A real base change whose target equals the painted value does match, so it is cancelled on every engine. The new test changes opacity before any such re-read, so it never sees this.

**MINOR** `llp/1061-press-feel-and-reduced-motion.rfc.md:271`. The re-read also drops a kept declaration when a token changes (`input-glue.js:460` via `repaintPress`). The paragraph names style, class, sheets, and scheme only.

**NIT** `llp/1061-press-feel-and-reduced-motion.rfc.md:291`. "With none, the layer takes the node's own clip and origin" is the background layer. A mask with no images still keeps `no-clip` (`input-glue.js:319`).

**NIT** `host/web/input-glue.js:362`. `state.background` is set only while `--exact-press-background-color` is present, so removing it leaves the flag set. `pressShown` still composites; the srgb copy falls back to `transparent`, so the reported color stays the node's.

Checked and holding: clip and origin are left alone when there is no background image (`:321`); `background-position-x`/`-y` and the mask axes fail the layer (`:294`, `:322`); `items()` skips quotes (`:307`); `pressShown` composites only a painted layer (`:404`); the style/class fingerprint plus `repaint(el, true)` for sheet, scheme, and token covers the stated inputs. The flush in `paint()` (`:278`) only cancels transitions that appear as the kept declaration is written back, and only for a property still in `state.kept` (one `later()` did not see running). It does not cancel an author transition that was already running. The hole is the poisoned `keptBase`, not that flush.

Verdict: not clear to ship — after a turn, the cancellation rule compares against the painted look.
