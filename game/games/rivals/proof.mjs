#!/usr/bin/env bun
// Rivals on a real host: the title, the training range (headshot, body shot,
// rocket splash, knife, mouse look), a duel the bot fights back in, a mid-fight
// save restored in a fresh process, and pause/restart. Web adds screenshots.
import {resolve} from 'node:path';
import {readFileSync, writeFileSync} from 'node:fs';
import { proof, axNames, decide } from "../../proof.mjs";

const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
async function checkNameplates(s, check, label) {
  const {nodes} = await s.layout(), canvas = nodes.find(n => n.testId === 'world');
  const plates = nodes.filter(n => /^plate-\d+$/.test(n.testId ?? ''));
  check(`${label}: visible nameplates fit their boxes and do not overlap`, plates.length > 0 && plates.every((p, i) =>
    p.w === 128 && p.h === 48 && p.x >= canvas.x + 4 && p.x + p.w <= canvas.x + canvas.w - 4 && p.y >= canvas.y + 4 &&
    plates.slice(i + 1).every(q => p.x + p.w + 6 <= q.x || q.x + q.w + 6 <= p.x || p.y + p.h + 6 <= q.y || q.y + q.h + 6 <= p.y)),
    plates.map(({testId, x, y, w, h}) => ({id:testId, x, y, w, h})));
}
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, say}) => {
  if (process.argv.includes('--playtest')) return process.argv.includes('--motor-check')
    ? motorCheck({open, out, check}) : playtest({open, out, say});
  if (process.argv.includes('--screenshot-only')) {
    const s = await open();
    check('screenshot uses web', host === 'web');
    await s.tap('ffa');
    await s.world('world').run(6000);
    await s.screenshot(resolve(out, 'game.png'));
    await s.close();
    return;
  }
  const s = await open();
  const title = await s.tree();
  const titleAx = await axNames(s);
  check('Duel is initially focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
  check('title offers duel, free-for-all, Mayhem and range', ['play', 'ffa', 'mayhem', 'range'].every(id => !!node(title, id)) && title.nodes.some(n => n.props?.text === 'RIVALS'));
  if (host !== 'linux') await s.screenshot(resolve(out, 'title.png'));
  check('world loads after a mode is chosen', !node(title, 'world'));
  await s.tap('range');
  const game = s.world('world');
  pin(0, await game.snapshot());
  check('initial HUD: full health, a full rifle', text(await s.tree(), 'hp') === '100' && text(await s.tree(), 'ammo') === '30 / 30');
  await game.run(400);
  await checkNameplates(s, check, 'Range');
  // The dummies' heads are at eye height: level aim is a headshot.
  await game.tap('KeyF');
  await game.run(50);
  const head = await game.get('bot-2', 'Fighter');
  check('level shot is a headshot for 36', head?.hp === 64, head?.hp);
  if (host === 'web') await s.screenshot(resolve(out, 'range.png'));
  const marked = await s.tree();
  check('hit marker and damage number', text(marked, 'crosshair') === '×' && marked.nodes.some(n => n.props?.text === '36'));
  check('visible target label follows damage', text(marked, 'target-label-3') === 'TARGET · bot-2' && text(marked, 'target-hp-3') === '64 HP');
  check('ammo counts down', text(marked, 'ammo') === '29 / 30');
  await game.hold('ArrowDown', 80);
  await game.run(300);
  await game.tap('KeyF');
  await game.run(50);
  check('lower aim is a body shot for 20', (await game.get('bot-2', 'Fighter'))?.hp === 44);
  // Mouse look: a held contact moved 100 points turns by the sensitivity.
  const before = (await game.get('player', 'Fighter')).yaw;
  await s.tap('world', {down:true, at:[300, 500]});
  // No duration: a timed move advances each host's clock differently, and
  // --compare-saves wants every host's sessions to end on the same tick.
  const moved = await s.pointer('move', {dx:100, dy:0, ms:0});
  await s.pointer('up');
  await game.run(50);
  const turned = before - (await game.get('player', 'Fighter')).yaw;
  check(`pointer drag turns the view (${moved.delivery})`, Math.abs(turned - 0.25) < 0.02, {turned, moved});
  // Turn back with the keys: 2.4 rad/s for the time that undoes the drag.
  await game.hold('ArrowLeft', turned / 2.4 * 1000);
  await game.run(50);
  await game.tap('Digit2');
  await game.run(400);
  check('rocket launcher selected', text(await s.tree(), 'weapon-name') === 'Rocket launcher');
  await game.tap('KeyF');
  await game.run(30);
  const flying = (await game.snapshot()).entities.filter(e => e.components?.Rocket).length;
  check('a rocket is in flight', flying === 1, flying);
  await game.run(600);
  const splashed = await Promise.all(['bot-1', 'bot-2', 'bot-3'].map(b => game.get(b, 'Fighter')));
  check('splash hits the neighbours too', splashed.filter(f => f.hp < 100 || !f.alive).length >= 2, splashed.map(f => f.hp));
  const feed = await s.tree();
  check('a kill reaches the kill feed', !!node(feed, 'feed') && feed.nodes.some(n => n.props?.text === '[rocket]'));
  await game.tap('Digit3');
  await game.hold('KeyW', 450);
  await game.run(300);
  await game.tap('KeyF');
  await game.run(100);
  const stabbed = (await game.snapshot()).entities.filter(e => e.components?.Fighter?.bot).map(e => e.components.Fighter);
  check('the knife lands', stabbed.some(f => f.hp === 55 || (f.hp < 100 && !f.alive) || f.deaths > 0), stabbed.map(f => [f.hp, f.deaths]));
  await s.close();

  // A duel: stand and spray toward the bot's spawn; it hunts, strafes and shoots back.
  const d = await open({fresh:true});
  await d.tap('play');
  const duel = d.world('world');
  await duel.key_down('KeyF');
  let elapsed = 0, incoming, detourAt = 0;
  while (elapsed < 9000 && !incoming) {
    await duel.run(100); elapsed += 100;
    const brain = await duel.get('bot-1', 'Brain');
    if (!detourAt && brain.detour_until > elapsed / 1000) {
      detourAt = elapsed;
      await duel.save(resolve(out, 'detour.world'));
    }
    incoming = text(await d.tree(), 'incoming-direction');
  }
  check('a received hit shows its direction and a compass heading', incoming?.startsWith('Hit from ') && !!node(await d.tree(), 'incoming-arrow') && text(await d.tree(), 'heading')?.startsWith('Facing '), incoming);
  if (host !== 'linux' && incoming) await d.screenshot(resolve(out, 'incoming.png'));
  await duel.run(9000 - elapsed);
  const me = await duel.get('player', 'Fighter'), bot = await duel.get('bot-1', 'Fighter');
  check('the bot fights back', me.hp < 100 || me.deaths > 0, me);
  const where = await duel.local_position('bot-1');
  check('the bot hunted off its spawn and fired', bot.shots > 0 && Math.hypot(where[0], where[2] + 18) > 3, {where, shots:bot.shots});
  await duel.key_up('KeyF');
  if (host === 'web') await d.screenshot(resolve(out, 'duel.png'));
  pin(1080, await duel.snapshot());
  await duel.save(resolve(out, 'fight.world'));
  const checkpoint = await duel.snapshot();
  await duel.hold('KeyD', 600);
  await duel.run(400);
  const continued = await duel.snapshot();
  await duel.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await d.tap('pause');
  const pausedAx = await axNames(d);
  check('Pause offers Restart', !!node(await d.tree(), 'restart') && (pausedAx.unavailable || pausedAx.name('restart') === 'Restart'));
  await d.tap('restart');
  const reset = await d.tree();
  check('Restart clears the score', text(reset, 'you-kills') === '0' && text(reset, 'rival-kills') === '0' && text(reset, 'hp') === '100');
  check('Restart returns focus to the game', node(reset, 'world')?.focused === true);
  await d.close();
  check('the hunting bot takes an obstacle detour before engaging', detourAt > 0, detourAt);
  if (detourAt) {
    const back = await open({fresh:true, world:resolve(out, 'detour.world')});
    await back.tap('play');
    const g = back.world('world');
    await g.run(9000 - detourAt);
    await g.key_up('KeyF');
    check('fresh process continues a mid-detour fight identically', JSON.stringify(await g.snapshot()) === JSON.stringify(checkpoint));
    await g.save(resolve(out, 'detour-continued.world'));
    check('mid-detour continuation saves are byte-identical', readFileSync(resolve(out, 'fight.world')).equals(readFileSync(resolve(out, 'detour-continued.world'))));
    await back.close();
  }
  const restored = await open({fresh:true, world:resolve(out, 'fight.world')});
  await restored.tap('play');
  const loaded = restored.world('world');
  check('a fresh process restores the fight', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await loaded.hold('KeyD', 600);
  await loaded.run(400);
  check('and continues identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await restored.close();

  // Seven bots: on the web, the screenshot of a fight in progress.
  const f = await open({fresh:true});
  await f.tap('ffa');
  check('the smaller free-for-all keeps its five-kill target', text(await f.tree(),'score-target') === 'First to 5');
  await f.world('world').key_down('KeyF');
  await f.world('world').run(2600);
  check('the free-for-all is a fight', (await f.world('world').snapshot()).entities.some(e => e.components?.Rocket || e.components?.Effect));
  if (host === 'web') await f.screenshot(resolve(out, 'game.png'));
  await f.close();

  // The maximum roster is a playable mode. Starts are distinct, and restoring
  // its busy fight must retain the same queries, bot decisions and respawns.
  const crowd = await open({fresh:true});
  await crowd.tap('mayhem');
  check('Mayhem names its longer round target', text(await crowd.tree(),'score-target') === 'First to 25');
  const mayhem = crowd.world('world');
  const starters = (await mayhem.snapshot()).entities.filter(e => e.components?.Fighter);
  check('Mayhem starts twenty-five fighters without overlapping capsules', starters.length === 25 && starters.every((a, i) => starters.slice(i + 1).every(b => {
    const p = a.components.Transform.position, q = b.components.Transform.position;
    return Math.hypot(p[0] - q[0], p[2] - q[2]) > 0.7;
  })));
  await mayhem.key_down('KeyF');
  await mayhem.run(2600);
  await checkNameplates(crowd, check, 'Mayhem');
  check('Mayhem is an active fight', (await mayhem.snapshot()).entities.filter(e => e.components?.Fighter).some(e => e.components.Fighter.deaths > 0));
  if (host !== 'linux') await crowd.screenshot(resolve(out, 'mayhem.png'));
  await mayhem.save(resolve(out, 'mayhem.world'));
  const finishMayhem = async session => {
    const g = session.world('world');
    await g.hold('KeyD', 600);
    await g.run(400);
    return g.snapshot();
  };
  const crowdedEnd = await finishMayhem(crowd);
  await mayhem.save(resolve(out, 'mayhem-continued.world'));
  pinSave('mayhem', resolve(out, 'mayhem-continued.world'));
  await crowd.close();
  const crowdBack = await open({fresh:true, world:resolve(out, 'mayhem.world')});
  await crowdBack.tap('mayhem');
  check('fresh process continues Mayhem identically', JSON.stringify(await finishMayhem(crowdBack)) === JSON.stringify(crowdedEnd));
  await crowdBack.world('world').save(resolve(out, 'mayhem-restored.world'));
  check('Mayhem continuation saves are byte-identical', readFileSync(resolve(out, 'mayhem-continued.world')).equals(readFileSync(resolve(out, 'mayhem-restored.world'))));

  // Five kills was previously the whole round. Keep playing past that point,
  // and save there so a fresh process must use the same longer win condition.
  const playing = crowdBack.world('world');
  const highScore = tree => Math.max(Number(text(tree,'you-kills')), Number(text(tree,'rival-kills')));
  let scores = await crowdBack.tree();
  for (let i = 0; i < 60 && highScore(scores) < 5; i++) {
    await playing.run(1000);
    scores = await crowdBack.tree();
  }
  check('Mayhem stays live past five kills', highScore(scores) >= 5 && !node(scores,'round-over'));
  await playing.save(resolve(out,'mayhem-five.world'));
  const five = await playing.snapshot();
  if (host !== 'linux') await crowdBack.screenshot(resolve(out,'mayhem-five.png'));
  const finishRound = async session => {
    const g = session.world('world');
    let tree = await session.tree();
    for (let i = 0; i < 180 && !node(tree,'round-over'); i++) {
      await g.run(1000);
      tree = await session.tree();
    }
    check('the full Mayhem round ends at twenty-five kills', !!node(tree,'round-over') && highScore(tree) === 25, highScore(tree));
    if (host !== 'linux') await session.screenshot(resolve(out,'mayhem-winner.png'));
    await g.run(4200);
    tree = await session.tree();
    check('the next round retains the Mayhem target', text(tree,'rounds')?.startsWith('Round 2 ·')
      && text(tree,'score-target') === 'First to 25' && !node(tree,'round-over'));
    return g.snapshot();
  };
  const nextRound = await finishRound(crowdBack);
  await playing.save(resolve(out,'mayhem-round.world'));
  pinSave('mayhem-round',resolve(out,'mayhem-round.world'));
  await crowdBack.close();
  const roundBack = await open({fresh:true,world:resolve(out,'mayhem-five.world')});
  await roundBack.tap('mayhem');
  check('a fresh process restores the fight past five kills', JSON.stringify(await roundBack.world('world').snapshot()) === JSON.stringify(five));
  check('the full round and restart continue identically', JSON.stringify(await finishRound(roundBack)) === JSON.stringify(nextRound));
  await roundBack.world('world').save(resolve(out,'mayhem-round-restored.world'));
  check('the longer round saves are byte-identical', readFileSync(resolve(out,'mayhem-round.world')).equals(readFileSync(resolve(out,'mayhem-round-restored.world'))));
  await roundBack.close();

  const training = await open({fresh:true});
  await training.tap('range');
  const range = training.world('world');
  await range.run(500);
  for (let i = 0; i < 3; i++) { await range.tap('KeyF'); await range.run(300); }
  check('a requested headshot elimination earns the first combo', text(await training.tree(), 'drill-score') === '150 points · ×1 combo · 1 targets');
  check('the next target is highlighted', text(await training.tree(), 'target-label-4') === 'TARGET · bot-3');
  await range.save(resolve(out, 'drill.world'));
  const finishDrill = async session => {
    const g = session.world('world');
    await g.run(2100);
    const at = await g.local_position('bot-2');
    check('the eliminated dummy returns to its lane', Math.abs(at[0]) < 0.05 && Math.abs(at[2] - 12) < 0.05, at);
    await g.hold('ArrowRight', 134);
    for (let i = 0; i < 3; i++) { await g.tap('KeyF'); await g.run(300); }
    check('switching to the requested target grows the combo', text(await session.tree(), 'drill-score') === '400 points · ×2 combo · 2 targets', text(await session.tree(), 'drill-score'));
    await g.run(30_000);
    check('the timed drill ends with a retry control', !!node(await session.tree(), 'drill-done') && !!node(await session.tree(), 'range-retry'));
  };
  await finishDrill(training);
  await range.save(resolve(out, 'drill-continued.world'));
  pinSave('drill', resolve(out, 'drill-continued.world'));
  const drillEnd = await range.snapshot();
  if (host !== 'linux') await training.screenshot(resolve(out, 'drill.png'));
  await training.tap('range-retry');
  check('retry starts a fresh thirty-second drill', text(await training.tree(), 'drill-clock') === 'Target drill · 30s' && text(await training.tree(), 'drill-score') === '0 points · ×0 combo · 0 targets');
  await training.close();
  const drillBack = await open({fresh:true, world:resolve(out, 'drill.world')});
  await drillBack.tap('range');
  await finishDrill(drillBack);
  check('fresh process continues the drill identically', JSON.stringify(await drillBack.world('world').snapshot()) === JSON.stringify(drillEnd));
  await drillBack.world('world').save(resolve(out, 'drill-restored.world'));
  check('drill continuation saves are byte-identical', readFileSync(resolve(out, 'drill-continued.world')).equals(readFileSync(resolve(out, 'drill-restored.world'))));
  await drillBack.close();

  // The rocket's ordinary self-splash supplies a wound without writing state.
  // Exercise the held HUD action, cancellation, and a saved keyboard hold.
  const care = await open({fresh:true});
  await care.tap('range');
  await rocketPractice(care);
  const caring = care.world('world');
  const wounded = Number(text(await care.tree(),'hp'));
  check('rocket practice leaves a living wound and an enabled bandage', wounded > 0 && wounded < 100 && node(await care.tree(),'bandage')?.props?.disabled === false, wounded);
  await care.tap('bandage', {down:true});
  await caring.run(500);
  check('holding the HUD button starts bandaging', text(await care.tree(),'bandage-status')?.startsWith('Keep holding'));
  await care.pointer('up');
  await caring.run(100);
  check('releasing early cancels without healing or spending the bandage', Number(text(await care.tree(),'hp')) === wounded && text(await care.tree(),'bandage-status') === 'Hold Q to bandage');
  await caring.key_down('KeyQ');
  await caring.run(700);
  await caring.save(resolve(out,'bandaging.world'));
  if (host !== 'linux') await care.screenshot(resolve(out,'bandaging.png'));
  const finishBandage = async session => {
    const g = session.world('world');
    await g.run(1000);
    check('the saved hold heals to full and spends one bandage', text(await session.tree(),'hp') === '100' && text(await session.tree(),'bandage-status') === 'Bandage used');
    await g.run(1700);
    await g.key_up('KeyQ'); await g.run(100);
    check('holding longer cannot use a second bandage', text(await session.tree(),'bandage-status') === 'Bandage used' && node(await session.tree(),'bandage')?.props?.disabled === true);
  };
  await finishBandage(care);
  await caring.save(resolve(out,'bandaged.world'));
  pinSave('bandage',resolve(out,'bandaged.world'));
  const cared = await caring.snapshot();
  if (host !== 'linux') await care.screenshot(resolve(out,'bandaged.png'));
  await care.close();
  const careBack = await open({fresh:true,world:resolve(out,'bandaging.world')});
  await careBack.tap('range');
  await finishBandage(careBack);
  check('fresh process continues the held bandage identically', JSON.stringify(await careBack.world('world').snapshot()) === JSON.stringify(cared));
  await careBack.world('world').save(resolve(out,'bandaged-restored.world'));
  check('bandage continuation saves are byte-identical', readFileSync(resolve(out,'bandaged.world')).equals(readFileSync(resolve(out,'bandaged-restored.world'))));
  await careBack.close();

  const reload = await open({fresh:true});
  await reload.tap('range');
  const loading = reload.world('world');
  await loading.run(100); await loading.tap('KeyF'); await loading.run(150);
  await loading.tap('KeyR'); await loading.run(800);
  check('reload meter exposes the early-finish window', text(await reload.tree(),'reload-status') === 'Press R now · quick reload'
    && !!node(await reload.tree(),'reload-meter') && text(await reload.tree(),'ammo') === 'Reloading…');
  await loading.save(resolve(out,'reload-window.world'));
  if (host !== 'linux') await reload.screenshot(resolve(out,'reload-window.png'));
  const finishReload = async session => {
    const g = session.world('world');
    await session.tap('reload'); await g.run(100);
    check('the HUD timing press refills before the normal deadline', text(await session.tree(),'ammo') === '30 / 30'
      && text(await session.tree(),'reload-status') === 'Quick reload!');
    await g.run(900); await g.tap('KeyF'); await g.run(200);
    check('the reloaded rifle fires normally', text(await session.tree(),'ammo') === '29 / 30');
    await session.tap('reload'); await g.run(200);
    await g.tap('KeyR'); await g.run(100);
    check('an early keyboard press reports the spent attempt', text(await session.tree(),'reload-status')?.startsWith('Missed'));
    await g.run(500); await g.tap('KeyR'); await g.run(100);
    check('pressing again cannot recover a missed window', text(await session.tree(),'ammo') === 'Reloading…'
      && text(await session.tree(),'reload-status')?.startsWith('Missed'));
    await g.run(900);
    check('a missed attempt still finishes the normal reload', text(await session.tree(),'ammo') === '30 / 30');
  };
  await finishReload(reload);
  await loading.save(resolve(out,'reloaded.world'));
  pinSave('quick-reload',resolve(out,'reloaded.world'));
  await reload.close();
  const reloadBack = await open({fresh:true,world:resolve(out,'reload-window.world')});
  await reloadBack.tap('range');
  await finishReload(reloadBack);
  await reloadBack.world('world').save(resolve(out,'reloaded-restored.world'));
  check('fresh process preserves the reload window and continuation bytes', readFileSync(resolve(out,'reloaded.world')).equals(readFileSync(resolve(out,'reloaded-restored.world'))));
  await reloadBack.close();
});

async function rocketPractice(session) {
  const g = session.world('world');
  await g.run(100);
  await g.tap('Digit2'); await g.run(300);
  await g.hold('ArrowDown',800);
  await g.tap('KeyF'); await g.run(1000);
  await g.hold('ArrowUp',800);
  await g.tap('Digit1'); await g.run(300);
}

async function visible(s) {
  const [tree, layout] = await Promise.all([s.tree(), s.layout()]);
  const canvas = layout.nodes.find(n => n.testId === 'world');
  const contacts = layout.nodes.filter(n => /^target-\d+$/.test(n.testId ?? '')).map(n => {
    const id = n.testId.slice(7);
    return {id, label:text(tree, `target-label-${id}`), hp:text(tree, `target-hp-${id}`),
      x:n.x - canvas.x, y:n.y - canvas.y};
  });
  return {tree, canvas, contacts};
}

// Queue the aim and trigger on one simulation tick. A held contact is a touch
// on web and a mouse on macOS; neither gets simulation time before KeyF joins it.
// The explicit trigger therefore makes one shot on either host. Integer points
// also match AppKit's raw mouse deltas. This is an authored aim assist, not Jev's
// visual perception or a measurement of a person's mouse flick.
async function aim(s, id) {
  const {canvas, contacts} = await visible(s);
  const c = contacts.find(c => c.id === id);
  if (!c) return false;
  const focal = canvas.h / 2 / Math.tan(74 * Math.PI / 360);
  const x = (c.x - canvas.w / 2) / focal;
  const y = (canvas.h / 2 - c.y) / focal;
  const yaw = Math.atan(x), pitch = Math.atan(y / Math.sqrt(1 + x * x));
  const dx = Math.round(yaw / 0.0025), dy = Math.round(-pitch / 0.0025);
  if (dx || dy) {
    await s.tap('world', {down:true, at:[canvas.w / 2, canvas.h / 2]});
    try { await s.pointer('move', {dx, dy, ms:0}); }
    finally { await s.pointer('up'); }
  }
  return true;
}

async function motorCheck({open, out, check}) {
  const s = await open();
  await s.tap('range');
  const game = s.world('world');
  await game.run(100);
  for (const [index, key, ms] of [[1,'ArrowRight',250], [2,'ArrowLeft',250], [3,'ArrowUp',70]]) {
    await game.hold(key, ms);
    const before = await game.get('player','Fighter'), started = s.now;
    check(`pointer aim ${index} sees the requested target`, await aim(s, '3'));
    check(`pointer aim ${index} queues without advancing simulation`, s.now === started);
    await game.tap('KeyF');
    await game.run(180);
    const after = await game.get('player','Fighter');
    check(`pointer aim ${index} fires exactly once`, after.shots === before.shots + 1, [before.shots,after.shots]);
    check(`pointer aim ${index} lands a headshot`, after.headshots === before.headshots + 1, [before.headshots,after.headshots]);
  }
  check('pointer motor clears the first target', text(await s.tree(),'drill-score') === '150 points · ×1 combo · 1 targets');
  await game.save(resolve(out,'aim.world'));
  await s.screenshot(resolve(out,'aim.png'));
  await s.close();
}

// An authored pointer motor aims from rendered nameplate positions. Jev
// chooses targets and tactics from the visible HUD and its own movement result;
// neither reads enemy world positions or writes the simulation. This measures
// decisions, not visual perception or human aim.
async function playtest({open, out, say}) {
  const timedReload = process.argv.includes('--reload-drill');
  const recovery = process.argv.includes('--recovery');
  const mayhem = process.argv.includes('--mayhem');
  const duel = !recovery && !timedReload && (process.argv.includes('--duel') || mayhem);
  const mode = timedReload ? 'reload' : recovery ? 'recovery' : mayhem ? 'mayhem' : duel ? 'duel' : 'drill';
  const s = await open();
  await s.tap(duel ? mayhem ? 'mayhem' : 'play' : 'range');
  const game = s.world('world');
  if (recovery) await rocketPractice(s);
  else await game.run(100);
  const transcript = resolve(out, `jev-${mode}-decisions.jsonl`);
  writeFileSync(transcript, '');
  const recent = [];
  const blocked = new Map();
  const moves = ['forward','back','left','right','jump_forward'];
  const actions = [];
  let quickReloads = 0, missedReloads = 0;
  let blindTurn = 0;
  for (let turn = 0; turn < (duel ? 128 : 96); turn++) {
    const {tree, contacts} = await visible(s);
    if (node(tree, 'drill-done') || node(tree, 'round-over')) break;
    const state = Object.fromEntries(['hp','ammo','weapon-name','you-kills','rival-kills','score-target','drill-clock','drill-score','drill-target','heading','incoming-direction']
      .map(id => [id, text(tree,id) ?? '']));
    if (recovery) Object.assign(state, {'bandage-hint':text(tree,'bandage-hint'), 'bandage-status':text(tree,'bandage-status')});
    if (timedReload) state['reload-status'] = text(tree,'reload-status');
    const reloading = state.ammo === 'Reloading…';
    const choices = {wait:'Wait half a second for a respawn, reload, or target to appear'};
    if (!node(tree, 'dead')) {
      if (recovery && node(tree,'bandage')?.props?.disabled === false) choices.bandage = 'Hold Q for 1.7 seconds to use the one bandage: recover up to 40 HP; damage or combat interrupts it';
      if (!timedReload || !reloading) for (const c of contacts) choices[`shoot_${c.id}`] = `Aim at ${c.label} (${c.hp}) and fire up to three shots with the selected weapon`;
      if (/^\d+ \/ \d+$/.test(state.ammo) && Number(state.ammo.split(' / ')[0]) < Number(state.ammo.split(' / ')[1])) choices.reload = timedReload
        ? 'Begin reloading, then follow the meter for a second press in green'
        : 'Reload the selected weapon, waiting two seconds';
      if (timedReload && reloading) Object.assign(choices, {
        wait_short:'Wait 0.1 seconds while watching the reload meter',
        quick_reload:'Press R again; green finishes early, a mistimed press spends the one timing attempt',
      });
      if (duel) Object.assign(choices, {
        forward:'Advance for half a second', left:'Strafe left for half a second',
        right:'Strafe right for half a second', back:'Retreat for half a second',
        jump_forward:'Jump and advance for half a second to clear low cover',
        scan:'Turn right about 35 degrees to search for an opponent',
        scan_left:'Turn left about 35 degrees to search for an opponent',
        turn_back:'Turn around 180 degrees to face an attack from behind',
        rifle:'Switch to the assault rifle for precise medium-range fire',
        rocket:'Switch to rockets for splash damage around cover',
      });
      // Two observed blocked attempts remove that motor command until the
      // player changes position or heading. A full blind turn likewise asks
      // for a new vantage point. Jev chooses among the remaining actions.
      for (const [action, failures] of blocked) if (failures >= 2) delete choices[action];
      if (contacts.length) blindTurn = 0;
      if (blindTurn >= 360) for (const action of ['scan','scan_left','turn_back']) delete choices[action];
    }
    const decision = await decide({state:{...state, contacts, recent, ...(duel ? {blockedActions:[...blocked].filter(([,n])=>n>=2).map(([name])=>name), blindTurnDegrees:Math.round(blindTurn)} : {})}, choices, transcript,
      goal:duel ? `Win the ${mayhem ? 'free-for-all against 24 bots' : 'duel'} while staying alive. Shoot visible opponents, reload when ammunition is low, and use the incoming-hit direction to turn toward attacks. The compass shows where you face. Recent movedMeters is your actual movement: a movement command under 0.3 metres hit an obstacle. Jump or strafe around it; do not repeat blocked steps. If repeated scanning finds nobody, move to a new position instead of spinning in place. The motor aims only at visible nameplates; it cannot see through cover. Avoid unnecessary weapon switching.`
        : timedReload ? 'Score as highly as possible in the thirty-second target drill, using timed reloads to spend less time without ammunition. Shoot the green TARGET and reload when needed. During a reload, the visible reload-status says when the meter is green: press R again then to refill immediately. You get one timing attempt per reload; a miss still finishes at the normal time. Wait in short steps to watch the meter. The motor aims at your chosen nameplate.'
        : recovery ? 'Recover to full health using your bandage after the rocket practice, then score as highly as possible in the remaining drill time. Shoot the green TARGET named in the HUD, avoiding other dummies to preserve the combo. Reload when needed; the motor aims at your chosen nameplate.'
        : 'Score as highly as possible in the thirty-second drill. Shoot the green TARGET named in the HUD, avoiding other dummies to preserve your combo. Reload when needed and wait if the requested dummy is respawning. The motor aims at your chosen nameplate.'});
    say(`JEV ${mode} ${turn+1}: ${decision.choice} · ${state['drill-score'] || `${state['you-kills']}–${state['rival-kills']} · ${state.hp} HP`}`);
    const started = s.now, triggers = [];
    const before = duel ? await game.local_position('player') : null;
    if (decision.choice.startsWith('shoot_')) {
      for (let shot = 0; shot < 3; shot++) {
        if (!await aim(s, decision.choice.slice(6))) break;
        triggers.push(s.now - started);
        await game.tap('KeyF');
        await game.run(180);
      }
    } else if (decision.choice === 'bandage') await game.hold('KeyQ',1700);
    else if (decision.choice === 'reload') { await game.tap('KeyR'); await game.run(timedReload ? 100 : 2300); }
    else if (decision.choice === 'quick_reload') { await game.tap('KeyR'); await game.run(100); }
    else if (decision.choice === 'wait_short') await game.run(100);
    else if (decision.choice === 'wait') await game.run(500);
    else if (decision.choice === 'jump_forward') { await game.tap('Space'); await game.hold('KeyW', 500); }
    else if (['rifle','rocket'].includes(decision.choice)) { await game.tap(decision.choice === 'rifle' ? 'Digit1' : 'Digit2'); await game.run(300); }
    else await game.hold({forward:'KeyW',left:'KeyA',right:'KeyD',back:'KeyS',scan:'ArrowRight',scan_left:'ArrowLeft',turn_back:'ArrowRight'}[decision.choice], decision.choice === 'turn_back' ? Math.PI / 2.4 * 1000 : decision.choice.startsWith('scan') ? 250 : 500);
    const after = duel ? await game.local_position('player') : null;
    if (timedReload) {
      const label = text(await s.tree(),'reload-status') ?? '';
      if (label === 'Quick reload!' && state['reload-status'] !== label) quickReloads++;
      if (label.startsWith('Missed') && !state['reload-status']?.startsWith('Missed')) missedReloads++;
    }
    actions.push({action:decision.choice, milliseconds:s.now-started, triggerMilliseconds:triggers});
    const moved = duel ? Math.round(Math.hypot(after[0]-before[0],after[2]-before[2])*100)/100 : 0;
    if (duel) {
      const turning = decision.choice.startsWith('scan') || decision.choice === 'turn_back';
      if (moved >= 0.3 || turning) blocked.clear();
      if (moves.includes(decision.choice) && moved < 0.3) blocked.set(decision.choice,(blocked.get(decision.choice)??0)+1);
      if (moved >= 1 || contacts.length) blindTurn = 0;
      else if (turning) blindTurn += decision.choice === 'turn_back' ? 180 : 2.4 * 0.25 * 180 / Math.PI;
    }
    recent.push({action:decision.choice, ammo:state.ammo, score:state['drill-score'], hp:state.hp, heading:state.heading, visible:contacts.length,
      ...(timedReload ? {reload:state['reload-status']} : {}),
      ...(duel ? {movedMeters:moved} : {})});
    if (recent.length > 4) recent.shift();
    if (turn === 9) await s.screenshot(resolve(out, `jev-${mode}-playing.png`));
  }
  const tree = await s.tree();
  const result = {mode, motor:'pointer', actions, score:text(tree,'drill-score'), done:!!node(tree,'drill-done') || !!node(tree,'round-over'), hp:text(tree,'hp'),
    ...(timedReload ? {quickReloads, missedReloads} : {}),
    playerKills:text(tree,'you-kills'), rivalKills:text(tree,'rival-kills'), target:text(tree,'score-target'), ...(recovery ? {bandage:text(tree,'bandage-status')} : {}), world:await game.snapshot()};
  say(`JEV outcome: ${JSON.stringify({...result,world:undefined,actions:undefined})}`);
  writeFileSync(resolve(out, `jev-${mode}-outcome.json`), JSON.stringify(result,null,2));
  await s.screenshot(resolve(out, `jev-${mode}.png`));
  await s.close();
}
