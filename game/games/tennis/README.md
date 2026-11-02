# Tennis

Singles against **Jev**, an AI that plans every one of its shots through the
Vercel AI Gateway while the engine plays them. First to four games.

| | |
|---|---|
| Move | WASD, arrow keys, or the Move stick |
| Forehand | **J** or the Forehand button — only on your racket-hand (right) side |
| Backhand | **K** or the Backhand button — only on your left |
| Timing | early pulls crosscourt, late pushes down the line; the wrong side or bad timing mishits, too far is a whiff |
| Stick at contact | ←/→ aims, ↑ flatter and deeper, ↓ slice |
| Serve | J or K tosses; press again at the top of the toss (the stick aims). Two faults lose the point |

The court is ITF singles (23.77 × 8.23 m, net 0.914 m at the strap, 1.07 m at the
singles sticks). The ball flies with gravity, drag and Magnus lift; topspin dips and
kicks, slice floats and skids; bounces have restitution and friction; the tape can
let a ball dribble over. The yellow ring on your side is where Jev's ball will
first bounce.

## Play

From the Exact2 root (`bun install --frozen-lockfile` once):

```sh
# Web, against Jev: the dev proxy holds the key; the page never sees it.
export AI_GATEWAY_API_KEY="$(sed -n 's/^[[:space:]]*AI_GATEWAY_API_KEY=//p' ~/Dropbox/APIKeys/vercel.txt)"
bun game/games/tennis/jev-proxy.mjs &      # 127.0.0.1:47913 → ai-gateway.vercel.sh
bun game/dev.mjs tennis                    # open the printed URL, choose Play Jev

# macOS, against Jev: the native host reads the key from its environment.
EXACT_APP_DIR=$PWD/game/games/tennis bun host/apple/build.mjs --run
```

Without a key or the proxy, choose **Play offline**: the deterministic fallback
policy plays every shot (Play Jev still works, and says `Jev unavailable`).

## How Jev plays

The world publishes one numbered question at a time in its HUD record (`hud.ask`).
Contract's `resource plan = jev(hud.ask)` hands it to the data source in `data/`,
which posts a Jev evaluation (`shot`, `target`, `aggression`, `approach_net`) and
returns the answer as the world's live `plan` argument. The world applies only the
answer whose id matches the open question, copies its probabilities into saved
state, and samples them with its seeded RNG — a run replays exactly from its inputs.
Questions go out early — your toss (Jev's return), Jev's own stroke (its next
shot), the start of Jev's service routine — so a plan has a ball's flight or two to
arrive. The HUD shows a plan when Jev plays it, with its probability, and says
`Jev thinking…` or `plan ready in 0.40 s` beside it. If no answer is in when the far
player must start its swing, a deterministic fallback plays and the HUD reads
`Jev late → fallback: …`.

Requests go straight to the gateway from a native host that has
`AI_GATEWAY_API_KEY`; otherwise, and always on the web, to the dev proxy. Both
origins are the data source's only grants.

## Prove

```sh
bun game/prove.mjs game/games/tennis                        # Linux, host-less
bun game/prove.mjs game/games/tennis --hosts linux,web      # plus Chrome pixels
bun game/games/tennis/proof.mjs web                         # artifacts/web/*.png
```

The proof drives the near player with keys only (serve, run to the contact hint the
world keeps on `world:near`, swing 19 ticks early), checks the side rule, pause,
a mid-rally save continued in a fresh process, a whole lost match, and — against a
scripted Jev it serves itself on the proxy's port — the question, the answer on the
HUD, Jev's intent driving the far player and a held answer falling back.
`logic/tests/sim.rs` covers the same without a host, with a scripted brain.

Live check, only with a key (real Jev, real time, Linux host):

```sh
export AI_GATEWAY_API_KEY=…   # as above
bun game/games/tennis/live.mjs
```

It plays points in real time and reports how many plans arrived, parsed, and beat
their deadline, with the latency distribution.
