#!/usr/bin/env bun
// A day and a night through real keys: chop the nearest tree, carry its logs to
// the fire, wait for dark, meet the Deer with the flashlight, and save mid-night.
import {resolve} from 'node:path';
import {readFileSync, writeFileSync} from 'node:fs';
import { proof, axNames, decide } from '../../proof.mjs';

const DAY = 80, DAWN = 8;
if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, say}) => {
  const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
  const s = await open();
  const text = async id => node(await s.tree(), id)?.props?.text;
  const game = s.world('world');
  // A key-only walker: hold the WASD pair toward the target in short steps.
  const walkTo = async (session, target, reach = 0.6, limit = 240) => {
    const g = session.world('world');
    for (let i = 0; i < limit; i++) {
      const p = await g.local_position('player');
      const dx = target[0] - p[0], dz = target[2] - p[2], d = Math.hypot(dx, dz);
      if (d < reach) return true;
      const keys = [];
      if (Math.abs(dx) > 0.2) keys.push(dx > 0 ? 'KeyD' : 'KeyA');
      if (Math.abs(dz) > 0.2) keys.push(dz > 0 ? 'KeyS' : 'KeyW');
      for (const key of keys) await session.type('world', {key, phase: 'down'});
      await g.run(Math.max(17, Math.min(200, d / 6 * 600)));
      for (const key of keys) await session.type('world', {key, phase: 'up'});
    }
    return false;
  };

  if (process.argv.includes('--playtest')) {
    // Jev chooses using only the visible HUD. The authored motor follows its
    // compass for one second; no inspected child positions or world writes.
    await s.tap('play');
    const transcript = resolve(out, 'jev-decisions.jsonl');
    writeFileSync(transcript, '');
    const recent = [];
    for (let turn = 0; turn < 48; turn++) {
      const tree = await s.tree();
      const state = Object.fromEntries(['objective','prompt','health','hunger','pack','fuel','battery','day','left','children']
        .map(id => [id, node(tree, id)?.props?.text ?? '']));
      if (node(tree, 'dead') || state.objective.startsWith('All children safe')) break;
      const heading = state.objective.match(/· (N|NE|E|SE|S|SW|W|NW) ·/u)?.[1];
      const choices = {wait:'Stay still for one second, allowing followers to catch up'};
      if (heading) choices.follow = 'Walk toward the compass objective for one second: find a child, or escort a follower home';
      if (state.prompt) choices.interact = `Press E now: ${state.prompt}`;
      if (!state.pack.includes('0 food')) choices.eat = 'Eat one carried food to restore hunger';
      choices.flashlight = 'Toggle the flashlight; it protects against creatures but uses battery';
      const decision = await decide({state:{...state, recent}, choices, transcript,
        goal:'Rescue both children and survive. Prefer taking a child in reach, otherwise follow the compass. Escort followers home for supplies. Eat when hungry. Daylight is limited.'});
      say(`JEV ${turn + 1}: ${decision.choice} · ${state.objective}`);
      recent.push({action:decision.choice, objective:state.objective, prompt:state.prompt});
      if (recent.length > 4) recent.shift();
      if (decision.choice === 'follow') {
        const keys = [...(heading.includes('N') ? ['KeyW'] : []), ...(heading.includes('S') ? ['KeyS'] : []),
          ...(heading.includes('E') ? ['KeyD'] : []), ...(heading.includes('W') ? ['KeyA'] : [])];
        try {
          for (const key of keys) await game.key_down(key);
          // Shorter strides near a child prevent stepping straight past reach.
          const distance = Number(state.objective.match(/· (\d+) m$/)?.[1] ?? 6);
          await game.run(Math.min(1000, Math.max(100, (distance - 1) / 6 * 1000)));
        } finally { for (const key of keys) await game.key_up(key); }
      } else if (decision.choice === 'wait') await game.run(1000);
      else {
        await game.tap({interact:'KeyE', eat:'KeyQ', flashlight:'KeyF'}[decision.choice]);
        await game.run(100);
      }
    }
    const tree = await s.tree();
    say(`JEV outcome: ${node(tree, 'children')?.props?.text}; ${node(tree, 'health')?.props?.text}`);
    // Outcomes remain observations: a weak model policy must not fake a proof.
    writeFileSync(resolve(out, 'jev-outcome.json'), JSON.stringify({
      children:node(tree, 'children')?.props?.text, health:node(tree, 'health')?.props?.text,
      objective:node(tree, 'objective')?.props?.text, world:await game.snapshot(),
    }, null, 2));
    await s.screenshot(resolve(out, 'jev-playtest.png'));
    await s.close();
    return;
  }

  if (process.argv.includes('--screenshot-only')) {
    // Pixels only: a day frame, then the same camp at night with the flashlight.
    check('screenshot uses web', host === 'web');
    await s.tap('play');
    await game.run(1500);
    await s.screenshot(resolve(out, 'day.png'));
    await game.run((DAY - DAWN + 2) * 1000);
    await game.hold('KeyW', 700);
    await game.tap('KeyF');
    await game.run(300);
    await s.screenshot(resolve(out, 'night.png'));
    await s.close();
    return;
  }
  const title = await s.tree();
  const titleAx = await axNames(s);
  check('Play is initially focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
  check('the title offers forest sizes', ['trees-1k', 'trees-5k', 'trees-20k', 'trees-100k'].every(id => node(title, id)));
  check('2k trees and Rapier are the default', node(title, 'choice')?.props?.text === '2000 trees · 8 wolves · Rapier collision · generated trees · 0 torches', node(title, 'choice')?.props?.text);
  await s.tap('play');
  pin(0, await game.snapshot());
  check('day one begins', await text('day') === 'Day 1 · Day', await text('day'));
  check('the census counts the forest', await text('census') === '2000 trees · 8 wolves', await text('census'));

  // Chop the tree nearest the fire.
  // Every page of the world, read at one tick: the standing tree nearest the fire.
  const trees = (await game.snapshot({all: true})).entities.filter(e => e.components?.Tree);
  check('every tree is readable past the first page', trees.length === 2000, trees.length);
  const tree = trees.map(e => e.components.Transform.position).sort((a, b) => Math.hypot(a[0], a[2]) - Math.hypot(b[0], b[2]))[0];
  check('walk to the nearest tree', await walkTo(s, [tree[0], 0, tree[2] + 1.2], 0.5), await game.local_position('player'));
  await game.run(300);
  check('a tree in reach offers a chop', await text('prompt') === 'E: chop', await text('prompt'));
  for (let i = 0; i < 3; i++) { await game.tap('KeyE'); await game.run(450); }
  check('three blows fell it', await text('census') === '1999 trees · 8 wolves', await text('census'));
  // Its two logs fall either side of the stump.
  for (const dx of [-0.7, 0.7]) {
    await walkTo(s, [tree[0] + dx, 0, tree[2] + 0.9], 0.45);
    await game.run(200);
    const prompt = await text('prompt');
    if (prompt === 'E: pick up log') { await game.tap('KeyE'); await game.run(100); }
  }
  check('both logs carried', await text('pack') === 'Carrying 2 logs · 0 scrap · 0 food', await text('pack'));
  const fuel = Number((await text('fuel')).match(/\d+/)[0]);
  check('walk back to the fire', await walkTo(s, [0, 0, 2.6], 0.6));
  await game.run(200);
  check('the fire asks for fuel', await text('prompt') === 'E: feed the fire', await text('prompt'));
  await game.tap('KeyE');
  await game.run(100);
  const fed = Number((await text('fuel')).match(/\d+/)[0]);
  check('feeding grows the fire', fed >= fuel + 20, {fuel, fed});
  check('logs are gone from the pack', await text('pack') === 'Carrying 0 logs · 0 scrap · 0 food');
  if (host === 'web') await s.screenshot(resolve(out, 'day.png'));

  // Wait for dark at the fire.
  const now = (await game.snapshot()).tick / 60;
  await game.run((DAY - DAWN - now + 3) * 1000);
  check('night falls', await text('day') === 'Day 1 · Night', await text('day'));
  const deer = await game.get('deer', 'Deer');
  check('the Deer stalks at night', JSON.stringify(deer).includes('Stalk'), deer);
  check('the Deer is visible', JSON.stringify(await game.get('deer', 'Visible')).includes('true'));
  const safe = Number((await text('radius')).match(/\d+/)[0]);
  const d = await game.local_position('deer');
  check('the Deer keeps out of the light', Math.hypot(d[0], d[2]) >= safe - 0.6, {d, safe});
  pin((await game.snapshot()).tick, await game.snapshot());
  await game.save(resolve(out, 'night.world'));
  const checkpoint = await game.snapshot();

  // Raise the flashlight and walk toward the Deer to the edge of the light.
  const meet = async (session) => {
    const g = session.world('world');
    const at = await g.local_position('deer');
    const r = Math.hypot(at[0], at[2]);
    await g.tap('KeyF');
    await walkTo(session, [at[0] / r * (safe + 1), 0, at[2] / r * (safe + 1)], 0.8);
    let stunned = false;
    for (let i = 0; i < 40 && !stunned; i++) {
      // Turn toward it with a two-tick step of the nearest of eight directions.
      const [p, q] = [await g.local_position('player'), await g.local_position('deer')];
      const a = Math.atan2(q[2] - p[2], q[0] - p[0]), keys = [];
      if (Math.abs(Math.cos(a)) > 0.38) keys.push(Math.cos(a) > 0 ? 'KeyD' : 'KeyA');
      if (Math.abs(Math.sin(a)) > 0.38) keys.push(Math.sin(a) > 0 ? 'KeyS' : 'KeyW');
      for (const key of keys) await session.type('world', {key, phase: 'down'});
      await g.run(34);
      for (const key of keys) await session.type('world', {key, phase: 'up'});
      await g.run(66);
      stunned = JSON.stringify(await g.get('deer', 'Deer')).includes('Stunned');
    }
    await g.run(500);
    return stunned;
  };
  check('the flashlight stuns the Deer', await meet(s), await game.get('deer', 'Deer'));
  check('the flashlight is on', (await text('battery')).startsWith('Flashlight on'), await text('battery'));
  if (host === 'web') await s.screenshot(resolve(out, 'night.png'));
  const continued = await game.snapshot();
  await game.save(resolve(out, 'continued.world'));
  pinSave('night', resolve(out, 'continued.world'));
  await s.close();

  // A fresh process restores mid-night and plays the same encounter identically.
  const r = await open({fresh: true, world: resolve(out, 'night.world')});
  await r.tap('play');
  const loaded = r.world('world');
  check('fresh process restores the night', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await meet(r);
  check('fresh process continues identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await r.close();

  // Rescue through real movement, then resume the escort in a fresh process.
  const rescue = await open({fresh:true});
  await rescue.tap('trees-1k');
  await rescue.tap('play');
  const escort = rescue.world('world');
  check('a visible compass gives the next objective', /Find a lost child · [NSEW]+ · \d+ m/.test(node(await rescue.tree(), 'objective')?.props?.text));
  const child = await escort.local_position('child-1');
  check('walk to a child', await walkTo(rescue, child, 1.6), await escort.local_position('player'));
  await escort.tap('KeyE');
  await escort.run(100);
  check('a child follows and the compass points home', JSON.stringify(await escort.get('child-1','Child')).includes('Following')
    && node(await rescue.tree(), 'objective')?.props?.text.startsWith('Escort 1 to the fire'));
  await escort.save(resolve(out, 'escort.world'));
  const checkpointEscort = await escort.snapshot();
  const returnHome = async session => {
    const world = session.world('world');
    const before = Number(node(await session.tree(), 'fuel')?.props?.text.match(/\d+/)?.[0]);
    const foods = async () => (await world.snapshot({all:true})).entities.filter(e => Object.hasOwn(e.components?.Item?.kind ?? {}, 'Food')).length;
    const food = await foods();
    check('walk the child home', await walkTo(session, [0,0,3], 0.6));
    await world.run(1500);
    check('rescue reaches the HUD', node(await session.tree(), 'children')?.props?.text === 'Children 1 of 2 rescued');
    check('rescue supplies add two food', await foods() === food + 2, {before:food,after:await foods()});
    check('rescue adds fuel despite the walk home', Number(node(await session.tree(), 'fuel')?.props?.text.match(/\d+/)?.[0]) > before);
    await world.run(1000);
    check('supplies are awarded once', await foods() === food + 2);
    return await world.snapshot();
  };
  const rescued = await returnHome(rescue);
  await escort.save(resolve(out, 'rescued.world'));
  pinSave('rescue', resolve(out, 'rescued.world'));
  if (host === 'web' || host === 'macos') await rescue.screenshot(resolve(out, 'rescue.png'));
  await rescue.close();
  const resume = await open({fresh:true, world:resolve(out, 'escort.world')});
  await resume.tap('trees-1k');
  await resume.tap('play');
  check('fresh process restores the escort', JSON.stringify(await resume.world('world').snapshot()) === JSON.stringify(checkpointEscort));
  check('rescue continues identically from the save', JSON.stringify(await returnHome(resume)) === JSON.stringify(rescued));
  await resume.world('world').save(resolve(out, 'rescued-restored.world'));
  check('rescue saves are byte-identical', readFileSync(resolve(out,'rescued.world')).equals(readFileSync(resolve(out,'rescued-restored.world'))));
  await resume.close();
});
