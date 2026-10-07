# Garden bakeoff — run 2026-10-06-r1

Builder grok-4.7 (high), exact2 at 7716302242. unity: not run: no Unity license on this Mac.

| | exact2 | three | godot |
|---|---:|---:|---:|
| finished (DONE.json) | yes | yes | yes |
| proof passed (judge rerun) | yes | yes | yes |
| wall clock, min | 105 | 89.3 | 69.4 |
| tokens, total | 40,032,853 | 14,012,794 | 12,366,035 |
| … uncached input | 1,092,860 | 952,634 | 361,277 |
| … output (incl. reasoning) | 346,969 | 307,776 | 214,550 |
| model calls | 328 | 115 | 99 |
| cost, USD | 8.4 | 3.6 | 2.7 |
| lines of game code | 3,095 | 3,451 | 2,950 |
| files | 13 | 13 | 9 |
| **fidelity** (0–100) | 81.4 | 93 | 87.5 |
| … world / rules / ui / proof % | 75/96.67/72.22/95 | 100/100/100/80 | 90/100/100/60 |
| … look (0–10) | 5 | 6 | 6 |
| **ergonomics, overall** (0–10) | 8 | 7 | 7 |
| … setup | 7 | 9 | 8 |
| … docs | 6 | 6 | 6 |
| … loop | 8 | 7 | 7 |
| … verification | 8 | 9 | 9 |
| … debugging | 7 | 6 | 6 |
| … assets | 8 | 6 | 5 |
| … ui | 8 | 7 | 8 |
| … api | 6 | 8 | 7 |
| … reliability | 6 | 9 | 6 |
| … visuals | 7 | 6 | 7 |
Fidelity is the judges' (Claude Opus, one per lane, adversarial, proof rerun from a clean
shell). Ergonomics is the builder's own holistic score after rereading its diary. The
self-score turn cost a further 0.50–0.65 M tokens per lane, not counted above. exact2's
wall clock includes two resumes after Grok ended its turn to wait on a backgrounded
Rust proof (LLP 1046.010 §6).

## Ranking — would I recommend it again for a game like this?

| rank | engine | fidelity | wall clock | cost | builder's score | why |
|---:|---|---:|---:|---:|---:|---|
| 1 | **three.js** | **93** | 89 min | $3.60 | 7 | The most complete game: every rule and the whole HUD right, camera follows, the real game plays end to end on real keys and clicks, and the most reliable tooling (9). Its proof leans on a test API instead of key events, and its offline check is close to a tautology. |
| 2 | **Godot 4.7** | 87.5 | **69 min** | **$2.73** | 7 | Fastest and cheapest, rules and HUD perfect, no addons, all as text. It loses on a camera that never follows the gardener (a miss its own diary didn't notice) and a proof that only exercises the simulation object, never the game scene; glTF import defaults (vertex colours off) cost real time. Within one rerun of first. |
| 3 | **exact2** | 81 | 105 min | $8.44 | **8** | The best proof story — the only one that drives the real game through keys and taps, with pins that held across the Linux and web hosts — and the builder liked it most. But it took the longest, used 2.9× Godot's tokens and 3.1× its cost, and shipped the most visible defects: invisible rain and snow, a grey meadow, Pause that banks paused time as "away", a gardener facing backwards, overflowing panels, and no real save across a reopen. The proof proved the harness path, not the player's. |
| — | Unity 6.3 | — | — | — | — | Not run: no license on this Mac. |

The builder's score and the judge disagree most on exact2 (8 vs. last on fidelity): the
verification loop the builder valued did not catch the defects a player would see.
