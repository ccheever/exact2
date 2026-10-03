#!/usr/bin/env bun
// Rivals on a real host: the title, the training range (headshot, body shot,
// rocket splash, knife, mouse look), a duel the bot fights back in, a mid-fight
// save restored in a fresh process, and pause/restart. Web adds screenshots.
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof } from "../../proof.mjs";

const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave}) => {
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
  check('Duel is initially focused and named', node(title, 'play')?.focused === true && node(title, 'play')?.accessibleName === 'Play');
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
  check('a kill reaches the kill feed', !!node(feed, 'feed-rocket'));
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
  check('Pause offers Restart', node(await d.tree(), 'restart')?.accessibleName === 'Restart');
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
});
