#!/usr/bin/env bun
// Rivals on a real host: the title, the training range (headshot, body shot,
// rocket splash, knife, mouse look), a duel the bot fights back in, a mid-fight
// save restored in a fresh process, and pause/restart. Web adds screenshots.
import {resolve} from 'node:path';
import {readFileSync, writeFileSync} from 'node:fs';
import { proof, axNames, decide } from "../../proof.mjs";

const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, say}) => {
  if (process.argv.includes('--playtest')) return playtest({open, out, say});
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
  check('title offers duel, free-for-all and range', ['play', 'ffa', 'range'].every(id => !!node(title, id)) && title.nodes.some(n => n.props?.text === 'RIVALS'));
  check('world loads after a mode is chosen', !node(title, 'world'));
  await s.tap('range');
  const game = s.world('world');
  pin(0, await game.snapshot());
  check('initial HUD: full health, a full rifle', text(await s.tree(), 'hp') === '100' && text(await s.tree(), 'ammo') === '30 / 30');
  await game.run(400);
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
  await duel.run(9000);
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
  await f.world('world').key_down('KeyF');
  await f.world('world').run(2600);
  check('the free-for-all is a fight', (await f.world('world').snapshot()).entities.some(e => e.components?.Rocket || e.components?.Effect));
  if (host === 'web') await f.screenshot(resolve(out, 'game.png'));
  await f.close();

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
});

// An authored keyboard motor aims from rendered nameplate positions. Jev
// chooses targets and tactics from the visible HUD; neither reads enemy world
// positions or writes the simulation. This measures decisions, not human aim.
async function playtest({open, out, say}) {
  const duel = process.argv.includes('--duel');
  const mode = duel ? 'duel' : 'drill';
  const s = await open();
  await s.tap(duel ? 'play' : 'range');
  const game = s.world('world');
  await game.run(100);
  const transcript = resolve(out, `jev-${mode}-decisions.jsonl`);
  writeFileSync(transcript, '');
  const recent = [];
  const visible = async () => {
    const [tree, layout] = await Promise.all([s.tree(), s.layout()]);
    const canvas = layout.nodes.find(n => n.testId === 'world');
    const contacts = layout.nodes.filter(n => /^target-\d+$/.test(n.testId ?? '')).map(n => {
      const id = n.testId.slice(7);
      return {id, label:text(tree, `target-label-${id}`), hp:text(tree, `target-hp-${id}`),
        x:n.x - canvas.x, y:n.y - canvas.y};
    });
    return {tree, canvas, contacts};
  };
  const aim = async id => {
    for (let pass = 0; pass < 2; pass++) {
      const {canvas, contacts} = await visible();
      const c = contacts.find(c => c.id === id);
      if (!c) return false;
      const focal = canvas.h / 2 / Math.tan(74 * Math.PI / 360);
      const x = (c.x - canvas.w / 2) / focal;
      const y = (canvas.h / 2 - c.y) / focal;
      const yaw = Math.atan(x), pitch = Math.atan(y / Math.sqrt(1 + x * x));
      if (Math.abs(yaw) > 0.009) await game.hold(yaw > 0 ? 'ArrowRight' : 'ArrowLeft', Math.min(250, Math.abs(yaw) / 2.4 * 1000));
      if (Math.abs(pitch) > 0.006) await game.hold(pitch > 0 ? 'ArrowUp' : 'ArrowDown', Math.min(250, Math.abs(pitch) / 1.44 * 1000));
    }
    return true;
  };
  for (let turn = 0; turn < (duel ? 64 : 96); turn++) {
    const {tree, contacts} = await visible();
    if (node(tree, 'drill-done') || node(tree, 'round-over')) break;
    const state = Object.fromEntries(['hp','ammo','weapon-name','you-kills','rival-kills','drill-clock','drill-score','drill-target']
      .map(id => [id, text(tree,id) ?? '']));
    const choices = {wait:'Wait half a second for a respawn, reload, or target to appear'};
    if (!node(tree, 'dead')) {
      for (const c of contacts) choices[`shoot_${c.id}`] = `Aim at ${c.label} (${c.hp}) and fire up to three shots with the selected weapon`;
      if (/^\d+ \/ \d+$/.test(state.ammo) && Number(state.ammo.split(' / ')[0]) < Number(state.ammo.split(' / ')[1])) choices.reload = 'Reload the selected weapon, waiting two seconds';
      if (duel) Object.assign(choices, {
        forward:'Advance for half a second', left:'Strafe left for half a second',
        right:'Strafe right for half a second', back:'Retreat for half a second',
        scan:'Turn right about 35 degrees to search for an opponent',
        rifle:'Switch to the assault rifle for precise medium-range fire',
        rocket:'Switch to rockets for splash damage around cover',
      });
    }
    const decision = await decide({state:{...state, contacts, recent}, choices, transcript,
      goal:duel ? 'Win the duel while staying alive. Shoot visible opponents, reload when ammunition is low, and move or turn to find opponents when none are visible. The motor aims only at visible nameplates; it cannot see through cover. Avoid unnecessary weapon switching.'
        : 'Score as highly as possible in the thirty-second drill. Shoot the green TARGET named in the HUD, avoiding other dummies to preserve your combo. Reload when needed and wait if the requested dummy is respawning. The motor aims at your chosen nameplate.'});
    say(`JEV ${mode} ${turn+1}: ${decision.choice} · ${state['drill-score'] || `${state['you-kills']}–${state['rival-kills']} · ${state.hp} HP`}`);
    recent.push({action:decision.choice, ammo:state.ammo, score:state['drill-score'], hp:state.hp});
    if (recent.length > 4) recent.shift();
    if (decision.choice.startsWith('shoot_')) {
      for (let shot = 0; shot < 3; shot++) {
        if (!await aim(decision.choice.slice(6))) break;
        await game.tap('KeyF');
        await game.run(180);
      }
    } else if (decision.choice === 'reload') { await game.tap('KeyR'); await game.run(2300); }
    else if (decision.choice === 'wait') await game.run(500);
    else if (['rifle','rocket'].includes(decision.choice)) { await game.tap(decision.choice === 'rifle' ? 'Digit1' : 'Digit2'); await game.run(300); }
    else await game.hold({forward:'KeyW',left:'KeyA',right:'KeyD',back:'KeyS',scan:'ArrowRight'}[decision.choice], decision.choice === 'scan' ? 250 : 500);
    if (turn === 9) await s.screenshot(resolve(out, `jev-${mode}-playing.png`));
  }
  const tree = await s.tree();
  const result = {mode, score:text(tree,'drill-score'), done:!!node(tree,'drill-done'), hp:text(tree,'hp'),
    playerKills:text(tree,'you-kills'), rivalKills:text(tree,'rival-kills'), world:await game.snapshot()};
  say(`JEV outcome: ${JSON.stringify({...result,world:undefined})}`);
  writeFileSync(resolve(out, `jev-${mode}-outcome.json`), JSON.stringify(result,null,2));
  await s.screenshot(resolve(out, `jev-${mode}.png`));
  await s.close();
}
