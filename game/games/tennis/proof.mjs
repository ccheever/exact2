#!/usr/bin/env bun
// Tennis, proven through the eight operations. The near player is driven by
// keys only (the same bot as logic/tests/sim.rs): it serves on the prompt,
// runs to the contact hint the world keeps on `world:near`, and presses J or
// K CONTACT ticks before the ball arrives. The offline session is pinned; the
// Jev session answers from a scripted local Jev on the proxy's origin, so the
// whole resource → HTTP → plan path runs on the real host without a network.
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof } from '../../proof.mjs';

const TICK = 1000 / 120 + 0.0001, CONTACT = 19, TOSS_HIT = 68 - CONTACT;
const name = v => typeof v === 'string' ? v : Object.keys(v ?? {})[0];
const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;

/** The near player: serve, return and rally through keys. */
export function bot(s, world) {
  let held = [];
  const hold = async want => {
    for (const key of held) if (!want.includes(key)) await world.key_up(key);
    for (const key of want) if (!held.includes(key)) await world.key_down(key);
    held = want;
  };
  const read = async () => {
    const near = await s.state('world:near');
    const ball = await world.get('ball', 'Ball');
    return {tick: near.tick, p: near.entity.components.Player, at: near.entity.components.Transform.position, ball};
  };
  const ticks = n => world.run(n * TICK);
  return {
    hold, read, ticks,
    /** Toss, then hit at the top of the toss. */
    async serve() {
      await hold([]);
      await world.tap('KeyJ');
      await ticks(TOSS_HIT);
      await world.tap('KeyJ');
      await ticks(CONTACT + 2);
    },
    /** Return the ball coming at you, if the world planned a contact. */
    async play({hand, aim = true} = {}) {
      let {tick, p, ball} = await read();
      if (!ball.live || name(ball.hitter) !== 'Far' || !(p.contact > tick)) return false;
      // Run in short legs toward the hint, easing off inside braking distance,
      // and stand still before the swing so the stick does not aim the shot.
      while (tick + CONTACT + 8 < p.contact) {
        const {at: now} = await read();
        const [dx, dz] = [p.goal[0] - now[0], p.goal[2] - now[2]];
        const want = [];
        if (dx > 0.15) want.push('KeyD'); else if (dx < -0.15) want.push('KeyA');
        // Depth matters less than width: only correct it when it is well off.
        if (dz < -0.4) want.push('KeyW'); else if (dz > 0.4) want.push('KeyS');
        await hold(want);
        const leg = Math.max(1, Math.min(Math.hypot(dx, dz) < 1 ? 2 : 5, p.contact - CONTACT - 8 - tick));
        await ticks(leg);
        tick += leg;
      }
      await hold([]);
      ({tick} = await read());
      if (p.contact - CONTACT - 1 > tick) await ticks(p.contact - CONTACT - 1 - tick);
      const stroke = hand ?? name(p.hand);
      // Aim for the open court: the stick is read at contact.
      const far = await world.local_position('far');
      if (aim) await hold([far[0] > 0 ? 'KeyA' : 'KeyD']);
      await world.tap(stroke === 'Backhand' ? 'KeyK' : 'KeyJ');
      await ticks(CONTACT + 2);
      await hold([]);
      return true;
    },
    /** Play until `done()` or `limit` ticks: serve on the prompt, return everything. */
    async rally(done, limit = 3000) {
      for (let spent = 0; spent < limit;) {
        if (await done()) return true;
        const tree = await s.tree();
        if ((text(tree, 'prompt') ?? '').startsWith('Your serve') || (text(tree, 'prompt') ?? '').startsWith('Second serve')) {
          await this.serve(); spent += TOSS_HIT + 2; continue;
        }
        if (await this.play()) { spent += 30; continue; }
        await ticks(4); spent += 4;
      }
      return done();
    },
  };
}

// `logs` answers with what is new since the last read; keep the whole journal per session.
const journals = new WeakMap();
export const lines = async s => {
  const all = journals.get(s) ?? [];
  all.push(...((await s.logs()).world ?? []).flatMap(chunk => chunk.lines ?? []));
  journals.set(s, all);
  return all;
};

/** A scripted Jev on the dev proxy's origin: answers, holds or fails on cue. */
function scriptedJev() {
  const requests = [], held = [];
  let mode = 'answer';
  const reply = (id, serve) => JSON.stringify({model: 'typesafe-ai/jev', answers: serve ? {
    shot: {type: 'choice', choice: 'kick', probabilities: {kick: 0.62, flat: 0.2, slice: 0.18}, confidence: 0.4},
    target: {type: 'choice', choice: 't', probabilities: {t: 0.7, wide: 0.2, body: 0.1}, confidence: 0.6},
    aggression: {type: 'score', score: 1.5, probabilities: {'1': 0.5, '2': 0.5}}, approach_net: {type: 'boolean', probability: 0}} : {
    shot: {type: 'choice', choice: 'slice', probabilities: {slice: 0.55, drive: 0.3, flat: 0.1, lob: 0.05, drop_shot: 0}, confidence: 0.3},
    target: {type: 'choice', choice: 'backhand', probabilities: {backhand: 0.8, forehand: 0.1, body: 0.05, open_court: 0.05}, confidence: 0.7},
    aggression: {type: 'score', score: 2.0, probabilities: {'2': 1}}, approach_net: {type: 'boolean', probability: 0}}, usage: {inputTokens: 600}});
  const cors = {'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type, authorization', 'access-control-allow-methods': 'POST, OPTIONS', 'access-control-allow-private-network': 'true'};
  const server = Bun.serve({hostname: '127.0.0.1', port: 47913, async fetch(request) {
    if (request.method === 'OPTIONS') return new Response(null, {status: 204, headers: cors});
    const body = await request.json();
    requests.push({body, authorization: request.headers.has('authorization')});
    const serve = Object.keys(body.questions.shot.criteria).includes('kick');
    const respond = () => new Response(reply(0, serve), {headers: {...cors, 'content-type': 'application/json'}});
    if (mode === 'hold') return new Promise(release => held.push(() => release(respond())));
    return respond();
  }});
  return {requests, set mode(m) { mode = m; }, release: () => held.splice(0).forEach(go => go()), stop: () => server.stop(true)};
}

if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave}) => {
  // Title screen: accessible, focused, both opponents offered.
  const s = await open({env: {AI_GATEWAY_API_KEY: ''}});
  const title = await s.tree();
  check('Play Jev is focused and named', node(title, 'play')?.focused === true && node(title, 'play')?.accessibleName === 'Play Jev');
  check('title offers the offline opponent', node(title, 'offline')?.accessibleName === 'Play offline');
  check('title explains the controls', ['TENNIS', 'J forehand · K backhand — on the correct side, in time', 'First to 4 games'].every(t => title.nodes.some(n => n.props?.text === t)));
  check('world loads after Play', !node(title, 'world'));

  // Offline: the pinned match.
  await s.tap('offline');
  const world = s.world('world'), me = bot(s, world);
  let hud = await s.tree();
  check('scoreboard starts level', text(hud, 'you-games') === '0' && text(hud, 'jev-games') === '0' && text(hud, 'you-points') === '0');
  check('you serve first', text(hud, 'prompt') === 'Your serve — J or K to toss, again to hit');
  check('offline opponent is named', text(hud, 'jev-line') === 'Offline opponent');
  check('touch controls are named buttons', ['Move', 'Forehand (J)', 'Backhand (K)', 'Pause'].every(n => hud.nodes.some(x => x.accessibleName === n)));
  pin(0, await world.snapshot());
  const court = await world.get('court', 'Mesh');
  check('singles court is 8.23 × 23.77 m', Math.abs(court.Plane.width - 8.23) < 1e-4 && Math.abs(court.Plane.depth - 23.77) < 1e-4, court);
  await me.serve();
  const served = await world.get('ball', 'Ball');
  check('the serve is struck', served.live && served.serve && name(served.hitter) === 'Near', served);
  await me.rally(async () => (await lines(s)).some(l => l.includes('far ')) , 600);
  check('Jev returns (offline fallback)', (await lines(s)).some(l => /far (Forehand|Backhand) \w+ → \w+ \(Offline\)/.test(l)));
  await me.rally(async () => (await lines(s)).some(l => /near (Forehand|Backhand) clean/.test(l)), 900);
  check('you hit a clean groundstroke', (await lines(s)).some(l => /near (Forehand|Backhand) clean timing/.test(l)));
  if (host === 'web') await s.screenshot(resolve(out, 'rally.png'));
  // The wrong stroke for the side mishits.
  const wrong = async () => {
    await me.rally(async () => { const r = await me.read(); return r.ball.live && name(r.ball.hitter) === 'Far' && r.p.contact > r.tick + 40 && !r.p.stretch; }, 3000);
    const {p} = await me.read();
    await me.play({hand: name(p.hand) === 'Forehand' ? 'Backhand' : 'Forehand', aim: false});
    return name(p.hand) === 'Forehand' ? 'Backhand' : 'Forehand';
  };
  const wrongHand = await wrong();
  check('the wrong stroke for the side mishits', (await lines(s)).some(l => l.includes(`near ${wrongHand} mishit`)));
  const scored = tree => (text(tree, 'you-points') !== '0' || text(tree, 'jev-points') !== '0') && !!text(tree, 'call');
  await me.rally(async () => scored(await s.tree()), 2400);
  hud = await s.tree();
  check('a point reaches the scoreboard', text(hud, 'you-points') !== '0' || text(hud, 'jev-points') !== '0', [text(hud, 'you-points'), text(hud, 'jev-points')]);
  check('the umpire calls it', /^(Out|Net|Winner|Unreturned|Ace|Double fault)/.test(text(hud, 'call') ?? ''), text(hud, 'call'));
  // Pause freezes the world; Resume continues.
  await s.tap('pause');
  const frozen = await world.snapshot();
  await world.run(500);
  check('pause freezes the world', (await world.snapshot()).hash === frozen.hash && node(await s.tree(), 'paused'));
  await s.tap('pause');
  check('resume hides the overlay', !node(await s.tree(), 'paused'));
  // Mid-rally save, continued in a fresh process.
  await me.rally(async () => { const r = await me.read(); return r.ball.live && name(r.ball.hitter) === 'Far' && r.p.contact > r.tick + 30; }, 2400);
  await me.hold([]);
  pin((await world.snapshot()).tick, await world.snapshot());
  await world.save(resolve(out, 'mid-rally.world'));
  const checkpoint = await world.snapshot();
  await me.play();
  await world.run(1500);
  const continued = await world.snapshot();
  await world.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await s.close();
  const again = await open({fresh: true, world: resolve(out, 'mid-rally.world'), env: {AI_GATEWAY_API_KEY: ''}});
  await again.tap('offline');
  const loaded = again.world('world');
  check('a fresh process restores the rally', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await bot(again, loaded).play();
  await loaded.run(1500);
  check('and continues it identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await again.close();

  // Jev: the resource carries the question over HTTP and the plan comes back.
  let jev;
  try { jev = scriptedJev(); } catch (error) { check('port 47913 is free for the scripted Jev', false, error.message); return; }
  try {
    const j = await open({fresh: true, env: {AI_GATEWAY_API_KEY: ''}});
    await j.tap('play');
    const jw = j.world('world'), jb = bot(j, jw);
    await j.world('world').tap('KeyJ');
    await jw.run(TICK * 2);
    check('the toss asks Jev', text(await j.tree(), 'jev-status') === 'Jev thinking…');
    // Wait in real time with the world clock stopped (the resource is pending
    // while the request is in flight): the answer then lands on a known tick.
    for (let i = 0; i < 300 && text(await j.tree(), 'jev-dot') === '◌'; i++) await Bun.sleep(10);
    await jw.run(TICK);
    const line = text(await j.tree(), 'jev-status') ?? '';
    check('Jev’s answer arrives and is kept secret', /^plan ready in 0\.\d\d s$/.test(line) && text(await j.tree(), 'jev-line') === 'Jev is watching', line);
    const asked = jev.requests[0]?.body;
    check('the request is a Jev evaluation', asked?.model === 'typesafe-ai/jev' && ['shot', 'target', 'aggression', 'approach_net'].every(k => asked.questions[k]), asked && Object.keys(asked.questions));
    check('it carries the situation and your tendencies', typeof asked?.state?.tendencies?.weaker_side === 'string' && /return of serve/.test(asked?.state?.decide ?? ''), asked?.state);
    check('a browser or proxy request carries no key', jev.requests.every(r => !r.authorization));
    await jw.run(TICK * (TOSS_HIT - 2));
    await jw.tap('KeyJ');
    await jb.rally(async () => (await lines(j)).some(l => /far (Forehand|Backhand) \w+ → \w+ \(Jev\)/.test(l)), 900);
    check('Jev’s intent drives the far player', (await lines(j)).some(l => /far (Forehand|Backhand) \w+ → \w+ \(Jev\)/.test(l)));
    check('the HUD reveals it at the swing, with Jev’s probability', /^Jev: (slice|drive|flat drive|lob|drop shot) → (your forehand|your backhand|your body|the open court) \(\d+%\)/.test(text(await j.tree(), 'jev-line') ?? ''), text(await j.tree(), 'jev-line'));
    const next = () => jev.requests.some(r => /you just hit a/.test(r.body.state?.incoming ?? ''));
    for (let i = 0; i < 100 && !next(); i++) { await j.tree(); await Bun.sleep(10); }
    check('striking, Jev is asked for its next shot', next(), jev.requests.map(r => r.body.state?.incoming));
    // Hold the next answer past the far player's commit point: the fallback plays.
    jev.mode = 'hold';
    await jb.rally(async () => /^Jev late → fallback: /.test(text(await j.tree(), 'jev-line') ?? ''), 2400);
    check('a late answer falls back, visibly', /^Jev late → fallback: /.test(text(await j.tree(), 'jev-line') ?? ''), text(await j.tree(), 'jev-line'));
    check('the journal records the late decision', (await lines(j)).some(l => /jev decision \d+ late after \d+ ms: fallback/.test(l)));
    if (host === 'web') await j.screenshot(resolve(out, 'jev.png'));
    // Close with the held answer still in flight: it can never land on a tick.
    await j.close();
  } finally { jev.release(); jev.stop(); }

  // Lose a whole match quickly (never return), then play again.
  const m = await open({fresh: true, env: {AI_GATEWAY_API_KEY: ''}});
  await m.tap('offline');
  const mw = m.world('world'), mb = bot(m, mw);
  for (let i = 0; i < 400 && !node(await m.tree(), 'match-over'); i++) {
    const prompt = text(await m.tree(), 'prompt') ?? '';
    if (prompt.startsWith('Your serve') || prompt.startsWith('Second serve')) await mb.serve();
    else await mw.run(700);
  }
  const over = await m.tree();
  check('Jev wins the match when you never return', text(over, 'winner') === 'Jev wins' && text(over, 'jev-games') === '4', [text(over, 'winner'), text(over, 'jev-games')]);
  check('Play again is focused', node(over, 'again')?.focused === true);
  if (host === 'web') await m.screenshot(resolve(out, 'match-over.png'));
  await m.tap('again');
  const fresh = await m.tree();
  check('Play again restarts the match', !node(fresh, 'match-over') && text(fresh, 'jev-games') === '0' && node(fresh, 'world')?.focused === true);
  await m.close();
});
