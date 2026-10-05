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
    const crafting = process.argv.includes('--build-camp');
    const survival = crafting || process.argv.includes('--survival');
    let stalled = 0;
    for (let turn = 0; turn < (crafting ? 96 : survival ? 128 : 48); turn++) {
      const tree = await s.tree();
      const state = Object.fromEntries(['objective','prompt','health','hunger','pack','drop-hint','fuel','battery','day','left','children','survived','camp-bearing','night-plan','night-supplies','build-status','build-cost']
        .map(id => [id, node(tree, id)?.props?.text ?? '']));
      const rescued = state.children === 'Children 2 of 2 rescued';
      const built = state['build-status'].startsWith('Built');
      const nights = Number(state.survived.match(/\d+/)?.[0]);
      if (node(tree, 'dead') || (crafting ? built && nights >= 1 : rescued && (!survival || nights >= 2))) break;
      const heading = state.objective.match(/· (N|NE|E|SE|S|SW|W|NW) ·/u)?.[1];
      const recovery = state.prompt.match(/^Axe recovering · (\d+\.\d) s/u);
      const wait = recovery ? Number(recovery[1]) * 1000 : survival && (rescued || crafting && built) ? 10_000 : 1000;
      const choices = {wait:recovery ? `Wait ${recovery[1]} seconds for the axe to recover`
        : `Stay still for ${wait / 1000} seconds, allowing time to pass and followers to catch up`};
      if (survival) {
        // Cardinal detours remain available when the visible distance shows
        // that two compass strides made no progress, instead of competing
        // with the near-target motor and repeatedly overshooting camp.
        if (stalled >= 2) Object.assign(choices, {north:'Detour north for one second around an obstacle',east:'Detour east for one second around an obstacle',
          south:'Detour south for one second around an obstacle',west:'Detour west for one second around an obstacle'});
        for (const id of ['track-rescue','track-fuel','track-food','track-camp','track-logs','track-scrap']) {
          const control = node(tree, id);
          if (control && !control.props?.disabled && control.props?.accessibilityPressed !== 'true') {
            choices[id] = `Choose the ${id.slice(6)} compass target`;
          }
        }
      }
      if (heading) choices.follow = 'Walk toward the visible compass objective for up to one second';
      if (state.prompt.startsWith('Hold E:')) choices.interact = `Hold E for one second: ${state.prompt}`;
      else if (state.prompt.startsWith('E:')) choices.interact = `Press E now: ${state.prompt}`;
      if (!state.pack.includes('0 food')) choices.eat = 'Eat one carried food to restore hunger';
      if (node(tree,'build-windbreak')?.props?.disabled === false) {
        choices.build = 'Build the camp windbreak: spend two carried logs and one scrap to halve fuel consumption permanently';
      }
      if (node(tree,'drop-supply')?.props?.disabled === false) {
        choices.drop = 'Drop the last packed supply on the ground for later pickup; it remains available through the material or food compass';
      }
      if (state.battery.startsWith('Flashlight on') || Number(state.battery.match(/(\d+)%/)?.[1]) > 5) {
        choices.flashlight = 'Toggle the flashlight; it protects against creatures but uses battery';
      }
      const decision = await decide({state:{...state, recent, stalledFollowSteps:stalled}, choices, transcript,
        goal:crafting ? 'Build a camp windbreak and survive until the first dawn. Gather two logs and one scrap using their compass buttons, return to camp and build instead of feeding those materials. Once built, keep the fire supplied and eat when hungry, then shelter at camp until dawn. Use only the visible compass and prompts.'
          : survival ? 'Rescue both children and survive two nights. Keep the campfire fueled, gather food and eat when hunger is low. After the children are safe, prepare supplies in daylight and shelter at camp at night. Use the visible compass and prompts; directions are W north, D east, S south, A west.'
          : 'Rescue both children and survive. Prefer taking a child in reach, otherwise follow the compass. Escort followers home for supplies. Eat when hungry. Daylight is limited.'});
      say(`JEV ${turn + 1}: ${decision.choice} · ${state.objective}`);
      recent.push({action:decision.choice, objective:state.objective, prompt:state.prompt});
      if (recent.length > 4) recent.shift();
      if (decision.choice === 'follow') {
        await followCompass(s, state.objective);
        const after = node(await s.tree(),'objective')?.props?.text ?? '';
        const distance = value => Number(value.match(/· (\d+) m$/)?.[1] ?? 0);
        stalled = after.split(' · ')[0] === state.objective.split(' · ')[0] && distance(after) >= distance(state.objective)
          ? stalled + 1 : 0;
      } else if (decision.choice === 'wait') await game.run(wait);
      else if (decision.choice === 'build') { await s.tap('build-windbreak'); await game.run(100); }
      else if (decision.choice === 'drop') { await s.tap('drop-supply'); await game.run(100); }
      else if (decision.choice.startsWith('track-')) { await s.tap(decision.choice); await game.run(100); stalled = 0; }
      else if (['north','east','south','west'].includes(decision.choice)) {
        await game.hold({north:'KeyW',east:'KeyD',south:'KeyS',west:'KeyA'}[decision.choice],1000);
        stalled = 0;
      }
      else {
        if (decision.choice === 'interact' && state.prompt.startsWith('Hold E:')) await game.hold('KeyE', 1000);
        else await game.tap({interact:'KeyE', eat:'KeyQ', flashlight:'KeyF'}[decision.choice]);
        await game.run(100);
      }
    }
    const tree = await s.tree();
    say(`JEV outcome: ${node(tree, 'children')?.props?.text}; ${node(tree, 'health')?.props?.text}`);
    // Outcomes remain observations: a weak model policy must not fake a proof.
    writeFileSync(resolve(out, 'jev-outcome.json'), JSON.stringify({
      children:node(tree, 'children')?.props?.text, health:node(tree, 'health')?.props?.text,
      objective:node(tree, 'objective')?.props?.text, survived:node(tree,'survived')?.props?.text,
      fuel:node(tree,'fuel')?.props?.text, hunger:node(tree,'hunger')?.props?.text, world:await game.snapshot(),
      nightPlan:node(tree,'night-plan')?.props?.text, nightSupplies:node(tree,'night-supplies')?.props?.text,
      windbreak:node(tree,'build-status')?.props?.text,
      player:await game.get('player','Player'), position:await game.local_position('player'),
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
  check('a tree in reach names the blows needed', await text('prompt') === 'Hold E: chop · 3 hits left', await text('prompt'));
  await game.tap('KeyE'); await game.run(100);
  check('the first blow shows recovery and progress', /^Axe recovering · 0\.[1-4] s · 2 hits left$/.test(await text('prompt')), await text('prompt'));
  if (host !== 'linux') await s.screenshot(resolve(out, 'chopping.png'));
  await game.save(resolve(out, 'chopping.world'));
  const choppingCheckpoint = await game.snapshot();
  const finishChopping = async session => {
    const g = session.world('world');
    const prompt = async () => node(await session.tree(), 'prompt')?.props?.text;
    // A press during recovery is still refused; the visible label tells why.
    await g.tap('KeyE'); await g.run(100);
    check('a rapid second press leaves two blows to go', (await prompt())?.endsWith('2 hits left'), await prompt());
    await g.run(250);
    check('the next swing is offered when recovery ends', await prompt() === 'Hold E: chop · 2 hits left', await prompt());
    await g.key_down('KeyE'); await g.run(100);
    check('the second blow leaves one', (await prompt())?.endsWith('1 hit left'), await prompt());
    await g.run(750);
    await g.key_up('KeyE'); await g.run(50);
    check('three accepted blows fell it', node(await session.tree(), 'census')?.props?.text === '1999 trees · 8 wolves');
    check('holding the axe does not pick up the dropped logs', node(await session.tree(), 'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
    return await g.snapshot();
  };
  const chopped = await finishChopping(s);
  await game.save(resolve(out, 'chopped.world'));
  pinSave('chopping', resolve(out, 'chopped.world'));
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

  const choppingBack = await open({fresh:true, world:resolve(out, 'chopping.world')});
  await choppingBack.tap('play');
  check('a fresh process restores mid-swing', JSON.stringify(await choppingBack.world('world').snapshot()) === JSON.stringify(choppingCheckpoint));
  check('the resumed swings continue identically', JSON.stringify(await finishChopping(choppingBack)) === JSON.stringify(chopped));
  await choppingBack.world('world').save(resolve(out, 'chopped-restored.world'));
  check('chopping continuation saves are byte-identical', readFileSync(resolve(out, 'chopped.world')).equals(readFileSync(resolve(out, 'chopped-restored.world'))));
  await choppingBack.close();

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

  // The new supply route uses only the HUD's bearing and normal keys. Save the
  // selected landmark before collecting it, then repeat from a fresh process.
  const supplies = await open({fresh:true});
  await supplies.tap('trees-1k');
  await supplies.tap('play');
  await supplies.tap('track-fuel');
  await supplies.world('world').run(100);
  const tracksFuel = async session => {
    const tree = await session.tree();
    return node(tree,'track-fuel')?.props?.accessibilityPressed === 'true'
      && node(tree,'objective')?.props?.text.startsWith('Gather log · ')
      && node(tree,'camp-bearing')?.props?.text.startsWith('Campfire · ');
  };
  check('fuel selection names a supply and keeps camp visible', await tracksFuel(supplies));
  await supplies.world('world').save(resolve(out,'trail.world'));
  const collectAndFeed = async session => {
    const world = session.world('world');
    const reach = async wanted => {
      for (let i = 0; i < 24; i++) {
        const tree = await session.tree();
        if (node(tree,'prompt')?.props?.text === wanted) return true;
        if (!await followCompass(session,node(tree,'objective')?.props?.text ?? '')) return false;
      }
      return false;
    };
    check('the fuel compass reaches a loose log', await reach('E: pick up log'));
    await world.tap('KeyE'); await world.run(100);
    check('the selected log can be collected', node(await session.tree(),'pack')?.props?.text === 'Carrying 1 logs · 0 scrap · 0 food');
    await session.tap('track-camp'); await world.run(100);
    check('the camp compass reaches the fire', await reach('E: feed the fire'));
    const before = Number(node(await session.tree(),'fuel')?.props?.text.match(/\d+/)?.[0]);
    await world.tap('KeyE'); await world.run(100);
    check('a compass supply run refuels camp', Number(node(await session.tree(),'fuel')?.props?.text.match(/\d+/)?.[0]) >= before + 11
      && node(await session.tree(),'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
    await session.tap('track-food'); await world.run(100);
    check('food can be selected after the fuel run', node(await session.tree(),'objective')?.props?.text.startsWith('Gather food · '));
    return await world.snapshot();
  };
  const supplied = await collectAndFeed(supplies);
  await supplies.world('world').save(resolve(out,'trail-continued.world'));
  pinSave('supplies',resolve(out,'trail-continued.world'));
  if (host === 'web' || host === 'macos') await supplies.screenshot(resolve(out,'supplies.png'));
  await supplies.close();
  const supplyBack = await open({fresh:true,world:resolve(out,'trail.world')});
  await supplyBack.tap('trees-1k'); await supplyBack.tap('play');
  check('a fresh process restores the selected supply landmark', await tracksFuel(supplyBack));
  check('the saved supply run continues identically', JSON.stringify(await collectAndFeed(supplyBack)) === JSON.stringify(supplied));
  await supplyBack.world('world').save(resolve(out,'trail-restored.world'));
  check('supply continuation saves are byte-identical', readFileSync(resolve(out,'trail-continued.world')).equals(readFileSync(resolve(out,'trail-restored.world'))));
  await supplyBack.close();

  // Finish the rescue, then follow the public preparation advice through dawn.
  // Keep this separate so the earlier rescue/supply continuations stay pinned.
  const shelter = await open({fresh:true,world:resolve(out,'rescued.world')});
  await shelter.tap('trees-1k'); await shelter.tap('play');
  const camp = shelter.world('world');
  check('walk to the second child', await walkTo(shelter, await camp.local_position('child-2'), 1.6));
  await camp.tap('KeyE'); await camp.run(100);
  check('escort the second child home', await walkTo(shelter,[0,0,2.5],0.5));
  await camp.run(2000);
  check('both children are safe', node(await shelter.tree(),'children')?.props?.text === 'Children 2 of 2 rescued');
  check('the public supply budget reports ready', node(await shelter.tree(),'night-supplies')?.props?.text === 'To dawn: fire ready · food ready');
  await camp.save(resolve(out,'shelter.world'));
  const untilDawn = async session => {
    const tree = await session.tree();
    const plan = node(tree,'night-plan')?.props?.text ?? '';
    check('prepared campers are told when the next milestone arrives', /^Shelter by the fire until dawn · \d+ s$/.test(plan), plan);
    await session.world('world').run(Number(plan.match(/(\d+) s$/)?.[1]) * 1000 + 100);
    const dawn = await session.tree();
    check('sheltering reaches the first dawn alive', node(dawn,'survived')?.props?.text === 'Nights survived: 1'
      && node(dawn,'health')?.props?.text === 'Health 100' && !node(dawn,'dead'));
    check('a new dawn asks for the next night’s supplies', node(dawn,'night-plan')?.props?.text === 'Gather fuel for the next dawn');
    return await session.world('world').snapshot();
  };
  if (host !== 'linux') await shelter.screenshot(resolve(out,'shelter.png'));
  const dawn = await untilDawn(shelter);
  pin(dawn.tick, dawn);
  await camp.save(resolve(out,'dawn.world'));
  pinSave('dawn',resolve(out,'dawn.world'));
  if (host !== 'linux') await shelter.screenshot(resolve(out,'dawn.png'));
  await shelter.close();
  const shelterBack = await open({fresh:true,world:resolve(out,'shelter.world')});
  await shelterBack.tap('trees-1k'); await shelterBack.tap('play');
  check('sheltering continues identically in a fresh process', JSON.stringify(await untilDawn(shelterBack)) === JSON.stringify(dawn));
  await shelterBack.world('world').save(resolve(out,'dawn-restored.world'));
  check('dawn continuation saves are byte-identical', readFileSync(resolve(out,'dawn.world')).equals(readFileSync(resolve(out,'dawn-restored.world'))));
  await shelterBack.close();

  // Collect the recipe through visible material compasses, then save before
  // returning to camp. A fresh process repeats the trip, build and fuel burn.
  const craft = await open({fresh:true});
  await craft.tap('trees-1k'); await craft.tap('play');
  const reachCraft = async (session, ready) => {
    for (let i = 0; i < 40; i++) {
      const tree = await session.tree();
      if (ready(tree)) return true;
      if (!await followCompass(session,node(tree,'objective')?.props?.text ?? '')) return false;
    }
    return false;
  };
  for (const kind of ['logs','logs','scrap']) {
    await craft.tap(`track-${kind}`); await craft.world('world').run(100);
    const label = kind === 'logs' ? 'log' : 'scrap';
    check(`the ${kind} compass reaches its material`, await reachCraft(craft,
      tree => node(tree,'prompt')?.props?.text === `E: pick up ${label}`));
    await craft.world('world').tap('KeyE'); await craft.world('world').run(100);
  }
  check('the windbreak recipe is carried', node(await craft.tree(),'pack')?.props?.text === 'Carrying 2 logs · 1 scrap · 0 food');
  check('building away from camp is disabled', node(await craft.tree(),'build-windbreak')?.props?.disabled === true);
  await craft.world('world').tap('KeyR'); await craft.world('world').run(100);
  check('an away build attempt keeps the materials', node(await craft.tree(),'pack')?.props?.text === 'Carrying 2 logs · 1 scrap · 0 food');
  await craft.world('world').save(resolve(out,'craft.world'));
  const buildCamp = async session => {
    const world = session.world('world');
    await session.tap('track-camp'); await world.run(100);
    check('returning to camp enables building', await reachCraft(session,
      tree => node(tree,'build-windbreak')?.props?.disabled === false));
    await session.tap('build-windbreak'); await world.run(100);
    const built = await session.tree();
    check('building consumes the recipe and reports the permanent benefit',
      node(built,'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food'
      && node(built,'build-status')?.props?.text === 'Built · Fire uses half the fuel'
      && node(built,'build-windbreak')?.props?.disabled === true);
    const before = Number(node(built,'fuel')?.props?.text.match(/\d+/)?.[0]);
    await world.tap('KeyR'); await world.run(20_000);
    const after = await session.tree();
    const spent = before - Number(node(after,'fuel')?.props?.text.match(/\d+/)?.[0]);
    check('the windbreak halves visible fuel consumption through twenty seconds', spent >= 3 && spent <= 7, spent);
    check('repeated building keeps the upgrade and empty pack',
      node(after,'build-status')?.props?.text === 'Built · Fire uses half the fuel'
      && node(after,'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
    return await world.snapshot();
  };
  const crafted = await buildCamp(craft);
  await craft.world('world').save(resolve(out,'windbreak.world'));
  pinSave('windbreak',resolve(out,'windbreak.world'));
  if (host !== 'linux') await craft.screenshot(resolve(out,'windbreak.png'));
  await craft.close();
  const craftBack = await open({fresh:true,world:resolve(out,'craft.world')});
  await craftBack.tap('trees-1k'); await craftBack.tap('play');
  check('saved materials build the same camp in a fresh process', JSON.stringify(await buildCamp(craftBack)) === JSON.stringify(crafted));
  await craftBack.world('world').save(resolve(out,'windbreak-restored.world'));
  check('built camp continuation saves are byte-identical', readFileSync(resolve(out,'windbreak.world')).equals(readFileSync(resolve(out,'windbreak-restored.world'))));

  // A ready camp can shelter before the rescue is finished. Follow its public
  // countdown, then prove the same night from the saved built camp.
  const shelterBuiltCamp = async session => {
    const tree = await session.tree();
    const plan = node(tree,'night-plan')?.props?.text ?? '';
    check('a built camp reports readiness while both children remain lost',
      /^Shelter by the fire until dawn · \d+ s$/.test(plan)
      && node(tree,'night-supplies')?.props?.text === 'To dawn: fire ready · food ready'
      && node(tree,'children')?.props?.text === 'Children 0 of 2 rescued', plan);
    await session.world('world').run(Number(plan.match(/(\d+) s$/)?.[1]) * 1000 + 100);
    const dawn = await session.tree();
    check('the windbreak camp reaches dawn alive without requiring a rescue',
      node(dawn,'survived')?.props?.text === 'Nights survived: 1'
      && node(dawn,'health')?.props?.text === 'Health 100' && !node(dawn,'dead')
      && node(dawn,'children')?.props?.text === 'Children 0 of 2 rescued');
    await session.tap('track-rescue'); await session.world('world').run(100);
    check('the children compass still offers the unfinished rescue after sheltering',
      node(await session.tree(),'objective')?.props?.text.startsWith('Find a lost child · '));
    return await session.world('world').snapshot();
  };
  const builtDawn = await shelterBuiltCamp(craftBack);
  await craftBack.world('world').save(resolve(out,'windbreak-dawn.world'));
  pinSave('windbreak-dawn',resolve(out,'windbreak-dawn.world'));
  if (host !== 'linux') await craftBack.screenshot(resolve(out,'windbreak-dawn.png'));
  await craftBack.close();
  const builtBack = await open({fresh:true,world:resolve(out,'windbreak.world')});
  await builtBack.tap('trees-1k'); await builtBack.tap('play');
  check('the saved built camp shelters identically in a fresh process',
    JSON.stringify(await shelterBuiltCamp(builtBack)) === JSON.stringify(builtDawn));
  await builtBack.world('world').save(resolve(out,'windbreak-dawn-restored.world'));
  check('built-camp dawn saves are byte-identical', readFileSync(resolve(out,'windbreak-dawn.world')).equals(readFileSync(resolve(out,'windbreak-dawn-restored.world'))));
  await builtBack.close();

  // Spare fuel stays useful: fill the fire, bring one more log, and leave it
  // at camp through the HUD. Later retrieve it, exercise G too, and refuel.
  const cache = await open({fresh:true,world:resolve(out,'craft.world')});
  await cache.tap('trees-1k'); await cache.tap('play');
  const cached = cache.world('world');
  await cache.tap('track-camp'); await cached.run(100);
  check('the carried recipe can instead be brought to the fire', await reachCraft(cache,
    tree => node(tree,'prompt')?.props?.text === 'E: feed the fire'));
  await cached.tap('KeyE'); await cached.run(100);
  check('the first fuel load fits in full', node(await cache.tree(),'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
  await cache.tap('track-logs'); await cached.run(100);
  check('the logs compass reaches the remaining loose log', await reachCraft(cache,
    tree => node(tree,'prompt')?.props?.text === 'E: pick up log'));
  await cached.tap('KeyE'); await cached.run(100);
  const spare = (await cached.snapshot({all:true})).entities.find(e => e.components?.Item?.carried);
  check('one spare log is carried', spare && Object.hasOwn(spare.components.Item.kind, 'Log'));
  await cache.tap('track-camp'); await cached.run(100);
  check('a nearly full fire explains why the spare log cannot fit', await reachCraft(cache,
    tree => node(tree,'prompt')?.props?.text === 'No room for carried fuel · G drops one supply'));
  await cached.tap('KeyE'); await cached.run(100);
  const full = await cache.tree();
  check('an extra feed press preserves the spare supply', node(full,'pack')?.props?.text === 'Carrying 1 logs · 0 scrap · 0 food'
    && node(full,'drop-hint')?.props?.text === 'G Drop log');
  if (host !== 'linux') await cache.screenshot(resolve(out,'spare-fuel.png'));
  await cache.tap('drop-supply'); await cached.run(100);
  const loose = (await cached.snapshot({all:true})).entities.find(e => e.id === spare.id);
  check('the drop button detaches the same item and empties the pack', loose && !loose.components.Item.carried
    && !Object.hasOwn(loose.components,'Parent') && node(await cache.tree(),'drop-supply')?.props?.disabled === true);
  await cache.tap('track-logs'); await cached.run(100);
  check('the material compass finds the deposited supply', node(await cache.tree(),'objective')?.props?.text === 'Gather log · within reach');
  await cached.save(resolve(out,'cached-supply.world'));
  if (host !== 'linux') await cache.screenshot(resolve(out,'cached-supply.png'));
  const retrieve = async session => {
    const world = session.world('world');
    await world.hold('KeyS',1000); await world.run(40_000);
    const left = (await world.snapshot({all:true})).entities.find(e => e.id === spare.id);
    check('the saved ground supply stays at camp while the player leaves',
      JSON.stringify(left?.components.Transform) === JSON.stringify(loose.components.Transform));
    check('the compass brings the player back to the cached log', await reachCraft(session,
      tree => node(tree,'prompt')?.props?.text === 'E: pick up log'));
    await world.tap('KeyE'); await world.run(100);
    await world.tap('KeyG'); await world.run(100);
    check('the keyboard drops the retrieved supply once', node(await session.tree(),'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
    await world.tap('KeyE'); await world.run(100);
    check('the same supply can be picked up again', node(await session.tree(),'pack')?.props?.text === 'Carrying 1 logs · 0 scrap · 0 food');
    await session.tap('track-camp'); await world.run(100);
    check('the fire accepts the spare log after burning room for it', await reachCraft(session,
      tree => node(tree,'prompt')?.props?.text === 'E: feed the fire'));
    const before = Number(node(await session.tree(),'fuel')?.props?.text.match(/\d+/)?.[0]);
    await world.tap('KeyE'); await world.run(100);
    check('stored fuel retains its full value', Number(node(await session.tree(),'fuel')?.props?.text.match(/\d+/)?.[0]) >= before + 11
      && node(await session.tree(),'pack')?.props?.text === 'Carrying 0 logs · 0 scrap · 0 food');
    return await world.snapshot();
  };
  const retrieved = await retrieve(cache);
  await cached.save(resolve(out,'cached-supply-continued.world'));
  pinSave('cached-supply',resolve(out,'cached-supply-continued.world'));
  await cache.close();
  const cacheBack = await open({fresh:true,world:resolve(out,'cached-supply.world')});
  await cacheBack.tap('trees-1k'); await cacheBack.tap('play');
  check('a fresh process retrieves and uses the saved supply identically',
    JSON.stringify(await retrieve(cacheBack)) === JSON.stringify(retrieved));
  await cacheBack.world('world').save(resolve(out,'cached-supply-restored.world'));
  check('cached-supply continuation saves are byte-identical', readFileSync(resolve(out,'cached-supply-continued.world')).equals(readFileSync(resolve(out,'cached-supply-restored.world'))));
  await cacheBack.close();
});

async function followCompass(session, objective) {
  const heading = objective.match(/· (N|NE|E|SE|S|SW|W|NW) ·/u)?.[1];
  if (!heading) return false;
  const game = session.world('world');
  const keys = [...(heading.includes('N') ? ['KeyW'] : []), ...(heading.includes('S') ? ['KeyS'] : []),
    ...(heading.includes('E') ? ['KeyD'] : []), ...(heading.includes('W') ? ['KeyA'] : [])];
  try {
    for (const key of keys) await game.key_down(key);
    const distance = Number(objective.match(/· (\d+) m$/)?.[1] ?? 6);
    await game.run(Math.min(1000, Math.max(100, (distance - 1) / 6 * 1000)));
  } finally { for (const key of keys) await game.key_up(key); }
  return true;
}
