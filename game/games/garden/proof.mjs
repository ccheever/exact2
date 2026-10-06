#!/usr/bin/env bun
// The garden on a real host: plant, grow, harvest, sell and buy through the
// HUD; a filled garden saved, restored in a fresh process and continued; the
// same save restored an hour later grows offline. `--scale` adds the
// measurements in the diary (entity ramp, long seeks, save sizes).
//
// Checks read what the world publishes for its HUD (logic/src/hud.rs: values,
// never sentences) and its inspected state, never the Contract's copy, so the
// HUD's wording (app.contract) is free to change. Where a check is about what
// the player reads (a control's accessible name, a visible hint), it asks that
// the text exists and names the published value, not that it is a particular
// sentence.
import {resolve} from 'node:path';
import {readFileSync, statSync, writeFileSync} from 'node:fs';
import { proof, axNames, decide } from '../../proof.mjs';

const EPOCH = Date.parse('2026-10-01T12:00:00Z');
const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
// A button's visible label: its own text, or the text it wraps (what Jev reads).
const label = (tree, id) => {
  const n = node(tree, id);
  return n?.props?.text ?? tree.nodes.find(m => m.parent === n?.id && m.props?.text != null)?.props.text;
};
const ms = t => Math.round(performance.now() - t);
// The world's published HUD record: the status, shop and backpack fields.
const hud = async s => (await s.state()).world?.find(w => w.name === 'world')?.published ?? {};
const seed = (h, id) => h.shop?.find(row => row.id === id);
// The seed in hand, as the shop rows publish it.
const holding = (h, id, owned) => seed(h, id)?.held === true && (owned === undefined || seed(h, id).owned === owned);
// The movement key a published way names ("south" is S).
const keyOf = way => ({north:'W', south:'S', east:'D', west:'A'})[way?.dir];
// A published fruit as the backpack names it ("Fed Gold Carrot").
const fruitLabel = f => `${f.fed ? 'Fed ' : ''}${f.muts.length ? `${f.muts.join(' ')} ` : ''}${f.crop}`;
// The player's tile ([x, z], from the published plot; null outside the
// garden) and the crop growing on it (a shop id; null when empty).
const plotOf = async (session, h) => {
  if (!h.plot?.inside) return {tile:null, crop:null};
  const tile = [h.plot.x, h.plot.z];
  const {entities} = await session.world('world').snapshot({all:true});
  const plant = entities.find(e => e.components?.Plant?.tile?.[0] === tile[0] && e.components.Plant.tile[1] === tile[1])?.components.Plant;
  return {tile, crop: plant ? h.shop[plant.kind]?.id : null, plant};
};
const at = (plot, x, z) => plot.tile?.[0] === x && plot.tile?.[1] === z;
// The crop the current market order asks for, by its published name.
const ordered = (h, id) => !!seed(h, id) && h.order?.crop === seed(h, id).name;

// The looks the Garden panel offers, by the button that chooses each.
const LOOKS = {classic:'look-classic', golden:'look-golden', storybook:'look-storybook', pass:'look-pass'};

if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, say, look}) => {
  const log = say ?? console.log;
  if (process.argv.includes('--playtest')) return playtest({open, out, log});
  if (process.argv.includes('--looks')) {
    // Each look chosen as a player chooses it, on a fresh garden at a fixed
    // epoch: the plot by the stall a second in, then a filled garden grown.
    for (const [name, button] of Object.entries(LOOKS)) await look(name, async frame => {
      const s = await open({epoch:EPOCH});
      await s.tap('play');
      const game = s.world('world');
      await s.tap('tools-tab');
      await s.tap(button);
      await s.tap('tools-tab');
      await game.run(1000);
      await frame('start', s);
      await s.tap('tools-tab');
      await s.tap('fill-100');
      await s.tap('tools-tab');
      await game.run(150_000);
      await frame('grown', s);
      await s.close();
    });
    return;
  }
  if (process.argv.includes('--screenshot-only')) {
    check('screenshot uses web', host === 'web');
    const s = await open({epoch:EPOCH});
    await s.tap('play');
    const game = s.world('world');
    await s.tap('tools-tab');
    await s.tap('fill-100');
    await game.run(150_000);
    await s.tap('shop-tab');
    await game.run(500);
    await s.screenshot(resolve(out, 'game.png'));
    // The overview of a garden the engine is being asked to carry.
    await s.tap('tools-tab');
    await s.tap('fill-10000');
    await game.run(34);
    await s.tap('zoom');
    await game.run(150_000);
    await s.tap('tools-tab');
    await game.run(500);
    await s.screenshot(resolve(out, 'overview.png'));
    await s.close();
    return;
  }
  if (process.argv.includes('--scale')) return scale({open, check, out, host, log});

  const s = await open({epoch:EPOCH});
  const title = await s.tree();
  const titleAx = await axNames(s);
  check('Play is focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || !!titleAx.name('play')));
  check('title explains the loop', title.nodes.some(n => /away/i.test(n.props?.text ?? '')));
  await s.tap('play');
  const game = s.world('world');
  pin(0, await game.snapshot());
  let h = await hud(s);
  check('starting purse and hand', h.sheckles === 20 && holding(h, 'carrot', 1), [h.sheckles, seed(h, 'carrot')]);
  let ax = await axNames(s);
  check('sheckles have an accessible name', ax.unavailable || ax.name('sheckles')?.includes(String(h.sheckles)), ax.name('sheckles'));
  const shopList = await s.tree('shop-list');
  check('the shop lists every seed', h.shop?.length > 0 && h.shop.every(row => node(shopList, `shop-${row.id}`)), h.shop?.map(row => row.id));

  await game.run(100);
  h = await hud(s);
  check('the prompt offers to plant', !!node(await s.tree(), 'prompt') && h.plants === 0 && h.prompt.what === 'plant' && h.prompt.crop === seed(h, 'carrot').name, h.prompt);
  const plotColor = snapshot => snapshot.entities.find(e => e.name === 'plot-north')?.components.Material.color;
  const emptyPlot = plotColor(await game.snapshot());
  check('the current plot has an outline', Array.isArray(emptyPlot));
  await game.tap('KeyE');
  await game.run(100);
  h = await hud(s);
  let t = await s.tree();
  check('E plants the carrot', h.plants === 1 && [19, 20].includes(h.prompt.s), [h.plants, h.prompt]);
  check('census counts it', h.plants === 1 && h.ripe === 0 && h.fruit === 0 && h.mutated === 0, [h.plants, h.ripe, h.fruit, h.mutated]);
  check('held seeds run out', h.held === '' && !h.shop.some(row => row.held), h.held);
  check('planted seeds disappear from shop inventory immediately', !node(t, 'equip-carrot'));
  const planted = await game.snapshot();
  const growingPlot = plotColor(planted);
  check('planting changes the plot outline', JSON.stringify(growingPlot) !== JSON.stringify(emptyPlot));
  const stem = planted.entities.find(e => e.components?.Plant)?.components.Transform.position;
  check('the new seedling stands beside the player', stem?.[0] > 0.6 && stem?.[2] < 0);
  if (host !== 'linux') await s.screenshot(resolve(out, 'planted.png'));
  await game.run(10_000);
  h = await hud(s);
  check('the countdown counts down', [9, 10].includes(h.prompt.s), h.prompt);
  const stage = (await game.snapshot()).entities.find(e => e.components?.Plant)?.components.Plant.stage;
  check('half grown is stage 2', stage === 2, stage);
  await game.run(10_500);
  h = await hud(s);
  check('a ripe carrot can be harvested', h.plants === 1 && h.ripe === 1 && h.prompt.what === 'harvest' && h.prompt.crop === seed(h, 'carrot').name, [h.ripe, h.prompt]);
  const ripePlot = plotColor(await game.snapshot());
  check('ripe fruit changes the outline again', JSON.stringify(ripePlot) !== JSON.stringify(growingPlot) && JSON.stringify(ripePlot) !== JSON.stringify(emptyPlot));
  if (host !== 'linux') await s.screenshot(resolve(out, 'ripe.png'));
  await game.tap('KeyE');
  await game.run(100);
  h = await hud(s);
  ax = await axNames(s);
  check('the backpack holds it', h.bag_count === 1 && (ax.unavailable || ax.name('bag-tab')?.includes('1')), [h.bag_count, ax.name('bag-tab')]);
  check('a carrot plant is gone after one harvest', h.plants === 0 && h.ripe === 0 && h.mutated === 0, [h.plants, h.ripe]);
  check('single harvest returns the empty outline', JSON.stringify(plotColor(await game.snapshot())) === JSON.stringify(emptyPlot));
  await s.tap('bag-tab');
  t = await s.tree();
  ax = await axNames(s);
  const fruit = h.bag[0];
  check('the backpack lists the fruit', !!node(t, `bag-${fruit?.id}`) && !!node(t, `sell-${fruit?.id}`)
    && (ax.unavailable || [fruitLabel(fruit.fruit), String(fruit.value)].every(part => ax.name(`sell-${fruit.id}`)?.includes(part))), ax.name(`sell-${fruit?.id}`));
  await s.tap('sell-all');
  await game.run(100);
  h = await hud(s);
  const purse = h.sheckles;
  check('selling pays', purse > 20 && h.bag_count === 0, [purse, h.bag_count]);
  check('an empty backpack says how to fill it', !!node(await s.tree(), 'bag-empty'));

  await s.tap('shop-tab');
  await s.tap('buy-carrot');
  await game.run(100);
  h = await hud(s);
  t = await s.tree();
  check('buying spends and fills the hand', h.sheckles === purse - 10 && holding(h, 'carrot', 1), [h.sheckles, seed(h, 'carrot')]);
  ax = await axNames(s);
  check('an owned seed offers to hold it', !!node(t, 'equip-carrot') && (ax.unavailable || ax.name('equip-carrot')?.includes(seed(h, 'carrot').name)), ax.name('equip-carrot'));
  check('an unaffordable seed is disabled', node(t, 'buy-grape')?.props?.disabled === true && seed(h, 'grape')?.affordable === false, node(t, 'buy-grape')?.props);
  // Two presses with no tick between them are two messages.
  await s.tap('buy-carrot');
  await s.tap('buy-carrot');
  await game.run(100);
  h = await hud(s);
  check('two presses between ticks buy two', holding(h, 'carrot', 3), seed(h, 'carrot'));

  await s.tap('tools-tab');
  await s.tap('fill-1000');
  await game.run(100);
  h = await hud(s);
  check('fill plants a thousand', h.plants === 1000, h.plants);
  await game.run(150_000);
  const grownHud = await hud(s);
  check('two and a half minutes ripen fruit', grownHud.ripe > 0 && grownHud.ripe <= grownHud.fruit, [grownHud.ripe, grownHud.fruit]);
  const mid = await game.snapshot({all:true});
  check('every page of a thousand plants and their fruit reads at one tick', mid.entities.length > 2000, mid.entities.length);
  pin(mid.tick, mid);
  await game.save(resolve(out, 'garden.world'));
  const checkpoint = await game.snapshot({all:true});
  await game.run(60_000);
  const continued = await game.snapshot({all:true});
  await game.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await s.close();

  const back = await open({fresh:true, world:resolve(out, 'garden.world'), epoch:EPOCH});
  await back.tap('play');
  const restored = back.world('world');
  check('a fresh process restores the garden', JSON.stringify(await restored.snapshot({all:true})) === JSON.stringify(checkpoint));
  await restored.run(60_000);
  check('and continues identically', JSON.stringify(await restored.snapshot({all:true})) === JSON.stringify(continued));
  await restored.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await back.close();

  const later = await open({fresh:true, world:resolve(out, 'garden.world'), epoch:EPOCH + 3_600_000});
  await later.tap('play');
  const grown = later.world('world');
  await grown.run(100);
  h = await hud(later);
  check('an hour later the garden grew while away', h.away.s > 0 && h.events > grownHud.events && !!node(await later.tree(), 'away'), [h.away, h.events]);
  const ripe = h.ripe;
  check('and every fruit is ripe', ripe > grownHud.ripe && ripe === h.fruit, [grownHud.ripe, ripe, h.fruit]);
  await later.tap('tools-tab');
  await later.tap('harvest-all');
  await grown.run(100);
  h = await hud(later);
  check('harvest all fills the backpack', h.bag_count === ripe, [h.bag_count, ripe]);
  await later.tap('bag-tab');
  const rows = (await later.tree('bag-list')).nodes.filter(n => n.props?.testId?.startsWith('bag-')).length;
  check('the virtualized backpack builds only the rows near the port', rows > 0 && rows < ripe, [rows, ripe]);
  if (host === 'web') await later.screenshot(resolve(out, 'away.png'));
  await later.close();

  // A market bonus unlocks the next crop immediately, without waiting for a
  // restock. Save before delivery and repeat the journey in a fresh process.
  const market = await open({fresh:true, epoch:EPOCH});
  await market.tap('play');
  const mg = market.world('world');
  await mg.run(100);
  h = await hud(market);
  check('the market names its first crop', h.orders === 0 && ordered(h, 'carrot') && !!node(await market.tree(), 'objective'), h.order);
  check('an unfilled order is disabled', node(await market.tree(), 'deliver')?.props?.disabled === true && h.order_ready === false);
  await mg.tap('KeyE');
  await mg.run(20_100);
  await mg.tap('KeyE');
  await mg.run(100);
  await mg.save(resolve(out, 'market.world'));
  const deliverAndGrow = async session => {
    const g = session.world('world');
    const before = (await hud(session)).sheckles;
    await session.tap('deliver');
    await g.run(100);
    let h = await hud(session), tree = await session.tree();
    check('delivery advances the order and pays fruit plus bonus', h.orders === 1 && ordered(h, 'strawberry') && h.sheckles > before + 30, [h.orders, h.order, h.sheckles]);
    check('bonus immediately enables strawberry purchase', node(tree, 'buy-strawberry')?.props?.disabled === false);
    await session.tap('buy-strawberry');
    await g.run(100);
    check('purchase feedback names the seed', (await hud(session)).last.crop === seed(h, 'strawberry').name);
    await g.tap('KeyE');
    await g.run(70_100);
    await g.tap('KeyE');
    await g.run(100);
    await session.tap('deliver');
    await g.run(100);
    h = await hud(session); tree = await session.tree();
    check('the second delivery funds blueberry', h.orders === 2 && ordered(h, 'blueberry') && h.sheckles >= 400 && node(tree,'buy-blueberry')?.props?.disabled === false, [h.orders, h.sheckles]);
    check('delivered fruit leaves the backpack', h.bag_count === 0);
    // A spare seed keeps the wrong crop equipped. The market offers the same
    // correction to a person and an agent, then explains why this tile is full.
    await session.tap('buy-carrot');
    await session.tap('buy-blueberry');
    await g.run(100);
    h = await hud(session); tree = await session.tree();
    check('the market offers to hold the requested owned seed', !!node(tree, 'equip-order') && h.order_seed === 'blueberry' && !!node(tree, 'order-hint'), h.order_seed);
    await session.tap('equip-order');
    await g.run(100);
    h = await hud(session);
    let plot = await plotOf(session, h);
    check('the occupied plot explains the next step', holding(h, 'blueberry') && plot.crop !== null && plot.crop !== 'blueberry' && h.order_hint.what === 'move', [plot.crop, h.order_hint]);
    await g.hold('KeyW', 500);
    await g.run(100);
    h = await hud(session); plot = await plotOf(session, h);
    check('walking finds an empty plot for blueberry', at(plot, 0, 1) && plot.crop === null && holding(h, 'blueberry'), [plot.tile, plot.crop]);
    await g.tap('KeyE');
    await g.run(100);
    h = await hud(session); plot = await plotOf(session, h);
    check('the planted plot and market agree on blueberry', at(plot, 0, 1) && plot.crop === 'blueberry' && ordered(h, 'blueberry') && h.order_seed === '', [plot.tile, plot.crop]);
    await g.run(100_100);
    if (host !== 'linux') await session.screenshot(resolve(out, 'blueberry.png'));
    await g.tap('KeyE');
    await g.run(100);
    await session.tap('deliver');
    await g.run(100);
    for (const [crop, order, key, growth] of [['tomato',4,'KeyD',150_100], ['corn',5,'KeyW',200_100]]) {
      h = await hud(session); tree = await session.tree();
      check(`order ${order} stocks its seed immediately`, h.orders === order - 1 && ordered(h, crop) && node(tree, `buy-${crop}`)?.props?.disabled === false);
      await session.tap(`buy-${crop}`);
      await g.run(100);
      await session.tap('equip-order');
      await g.run(100);
      await g.hold(key, 500);
      await g.run(100);
      await g.tap('KeyE');
      await g.run(growth);
      await g.tap('KeyE');
      await g.run(100);
      check(`order ${order} can be delivered`, node(await session.tree(), 'deliver')?.props?.disabled === false && (await hud(session)).order_ready === true);
      await session.tap('deliver');
      await g.run(100);
    }
    h = await hud(session); tree = await session.tree();
    check('all five orders finish without a stock wait', h.orders === 5 && !node(tree, 'deliver'), h.orders);
  };
  await deliverAndGrow(market);
  const marketEnd = await mg.snapshot();
  await mg.save(resolve(out, 'market-continued.world'));
  pinSave('market', resolve(out, 'market-continued.world'));
  if (host !== 'linux') await market.screenshot(resolve(out, 'market.png'));
  await market.close();
  const marketBack = await open({fresh:true, world:resolve(out, 'market.world'), epoch:EPOCH});
  await marketBack.tap('play');
  await deliverAndGrow(marketBack);
  check('fresh process continues market rewards identically', JSON.stringify(await marketBack.world('world').snapshot()) === JSON.stringify(marketEnd));
  await marketBack.world('world').save(resolve(out, 'market-restored.world'));
  check('market continuation saves are byte-identical', readFileSync(resolve(out, 'market-continued.world')).equals(readFileSync(resolve(out, 'market-restored.world'))));
  await marketBack.close();

  // Reproduce the native Jev run's north-boundary trap using ordinary keys.
  // The return trip reads the public prompt, including after a fresh restore.
  const lost = await open({fresh:true, epoch:EPOCH});
  await lost.tap('play');
  const lg = lost.world('world');
  await lg.hold('KeyD', 500);
  await lg.run(100);
  await lg.hold('KeyW', 4_000);
  await lg.run(100);
  h = await hud(lost);
  check('outside the north edge gives the direction back', (await plotOf(lost, h)).tile === null && keyOf(h.prompt.way) === 'S' && !!node(await lost.tree(), 'prompt'), h.prompt);
  if (host !== 'linux') await lost.screenshot(resolve(out, 'lost.png'));
  await lg.save(resolve(out, 'outside.world'));
  const returnAndPlant = async session => {
    const g = session.world('world');
    for (let step = 0; step < 20; step++) {
      const h = await hud(session);
      if ((await plotOf(session, h)).tile) break;
      const key = keyOf(h.prompt.way);
      check('the return prompt names a movement key', !!key, h.prompt);
      if (!key) break;
      await g.hold(`Key${key}`, 500);
      await g.run(100);
    }
    let h = await hud(session), plot = await plotOf(session, h);
    check('following the prompt reaches a usable plot', at(plot, 1, 5) && plot.crop === null && holding(h, 'carrot', 1), [plot.tile, plot.crop, h.prompt]);
    await g.tap('KeyE');
    await g.run(100);
    check('the recovered player can plant', (await hud(session)).plants === 1);
    await session.tap('buy-carrot');
    await g.run(100);
    h = await hud(session); plot = await plotOf(session, h);
    check('an occupied edge plot points to an empty plot', !!keyOf(h.planting.way) && plot.crop === 'carrot' && plot.plant.stage < 4 && !!node(await session.tree(), 'planting'), [h.planting, plot.crop]);
    if (host !== 'linux') await session.screenshot(resolve(out, 'empty-direction.png'));
    for (let step = 0; step < 20; step++) {
      const h = await hud(session);
      if ((await plotOf(session, h)).crop === null) break;
      const key = keyOf(h.planting.way);
      check('the empty-plot hint names a movement key', !!key, h.planting);
      if (!key) break;
      await g.hold(`Key${key}`, 500);
      await g.run(100);
    }
    h = await hud(session); plot = await plotOf(session, h);
    check('following the empty-plot hint reaches a planting tile', plot.tile !== null && plot.crop === null && holding(h, 'carrot', 1) && h.planting.what === '' && !node(await session.tree(), 'planting'), [plot.tile, h.planting]);
    await g.tap('KeyE');
    await g.run(100);
    check('the guided player plants a second crop', (await hud(session)).plants === 2);
    return g.snapshot();
  };
  const returned = await returnAndPlant(lost);
  await lg.save(resolve(out, 'returned.world'));
  pinSave('recovery', resolve(out, 'returned.world'));
  if (host !== 'linux') await lost.screenshot(resolve(out, 'returned.png'));
  await lost.close();
  const lostBack = await open({fresh:true, world:resolve(out, 'outside.world'), epoch:EPOCH});
  await lostBack.tap('play');
  check('a fresh process follows the same return guidance', JSON.stringify(await returnAndPlant(lostBack)) === JSON.stringify(returned));
  await lostBack.world('world').save(resolve(out, 'returned-restored.world'));
  check('returning and planting saves are byte-identical', readFileSync(resolve(out, 'returned.world')).equals(readFileSync(resolve(out, 'returned-restored.world'))));
  await lostBack.close();

  // Care is a saved action: accelerate growth, leave its plot, return using
  // public barrel guidance and refill, then harvest in a fresh-process replay.
  const care = await open({fresh:true, epoch:EPOCH});
  await care.tap('play');
  const cg = care.world('world');
  await cg.run(100);
  await cg.tap('KeyE'); await cg.run(100);
  await care.tap('buy-carrot'); await cg.run(100);
  check('the tall text HUD is present while watering', !!node(await care.tree(),'planting'));
  check('a growing plot offers watering', node(await care.tree(), 'water')?.props?.disabled === false && (await hud(care)).water_ready === true);
  await care.tap('water'); await cg.run(100);
  h = await hud(care);
  const wateredPlant = (await plotOf(care, h)).plant;
  check('one dose accelerates growth and disables another', h.water === 2 && wateredPlant?.watered === true
    && h.water_ready === false && node(await care.tree(),'water')?.props?.disabled === true, [h.water, wateredPlant?.watered]);
  const wateringCue = (await cg.resources()).Feedback;
  check('watering shows its can and live droplets', (await cg.get('watering-can','Visible'))?.[0] === true
    && (await cg.get('feedback-water','Emitter'))?.state?.alive > 0);
  check('successful watering starts its sound', (await care.state()).world?.find(w=>w.name==='world')?.audio?.voices?.some(v=>v.sound==='water'));
  if (host !== 'linux') await care.screenshot(resolve(out,'watering.png'));
  await cg.tap('KeyQ'); await cg.run(100);
  check('the keyboard cannot spend a second dose on the same growth', (await hud(care)).water === 2);
  check('refused watering does not restart its gesture', (await cg.resources()).Feedback?.began === wateringCue?.began);
  await cg.hold('KeyD',1000); await cg.run(100);
  check('finished care sounds leave no retained voices', (await care.state()).world?.find(w=>w.name==='world')?.audio?.voices?.length === 0);
  check('refilling away from the barrel is disabled', node(await care.tree(),'refill')?.props?.disabled === true && (await hud(care)).refill_ready === false);
  await cg.save(resolve(out,'watering.world'));
  const finishCare = async session => {
    const g = session.world('world');
    for (let step=0;step<20;step++) {
      const h = await hud(session);
      if (h.refill_ready) break;
      const key = keyOf(h.barrel.way);
      check('the barrel hint names a movement key', !!key, h.barrel);
      if (!key) break;
      await g.hold(`Key${key}`,200); await g.run(100);
    }
    await g.tap('KeyR'); await g.run(100);
    check('R refills all three doses near the barrel', (await hud(session)).water === 3);
    for (let step=0;step<10;step++) {
      const h = await hud(session);
      if ((await plotOf(session, h)).tile) break;
      const key = keyOf(h.prompt.way);
      if (!key) break;
      await g.hold(`Key${key}`,200); await g.run(100);
    }
    const left = 19_000 - (await g.snapshot()).tick * 1000 / 30;
    if (left > 0) await g.run(left);
    const h = await hud(session);
    check('the watered carrot ripens before twenty seconds', h.ripe === 1 && (await plotOf(session, h)).crop === 'carrot' && (await g.snapshot()).tick < 600, [h.ripe, h.prompt]);
    await g.tap('KeyE'); await g.run(100);
    check('the accelerated harvest fills the backpack once', (await hud(session)).bag_count === 1);
    check('harvest carries the fruit while its reward sound plays', (await g.get('picked-fruit','Visible'))?.[0] === true
      && (await session.state()).world?.find(w=>w.name==='world')?.audio?.voices?.some(v=>v.sound==='harvest'));
    return g.snapshot();
  };
  const watered = await finishCare(care);
  await cg.save(resolve(out,'watering-continued.world'));
  pinSave('watering',resolve(out,'watering-continued.world'));
  await care.close();
  const careBack = await open({fresh:true,world:resolve(out,'watering.world'),epoch:EPOCH});
  await careBack.tap('play');
  check('watered growth and barrel travel continue identically', JSON.stringify(await finishCare(careBack)) === JSON.stringify(watered));
  await careBack.world('world').save(resolve(out,'watering-restored.world'));
  check('watered continuation saves are byte-identical',readFileSync(resolve(out,'watering-continued.world')).equals(readFileSync(resolve(out,'watering-restored.world'))));
  await careBack.close();

  // Recycle a real harvest, feed the next crop through its HUD control, then
  // resume the saved growing plot in another host process.
  const compost = await open({fresh:true, epoch:EPOCH});
  await compost.tap('play');
  const fg = compost.world('world');
  await fg.run(100);
  await compost.tap('buy-carrot'); await fg.run(100);
  await fg.tap('KeyE'); await fg.run(20_100);
  await fg.tap('KeyE'); await fg.run(100);
  await compost.tap('bag-tab');
  h = await hud(compost);
  const harvested = h.bag[0];
  check('a harvested fruit offers a named compost trade', node(await compost.tree(),`compost-${harvested?.id}`)?.props?.accessibilityLabel?.includes(fruitLabel(harvested.fruit)), harvested);
  const cash = h.sheckles;
  await compost.tap(`compost-${harvested.id}`);
  const pendingCompost = (await compost.state()).world?.find(w => w.name === 'world')?.input?.pending;
  check('state exposes the compost message waiting for a tick', pendingCompost?.message === 1, pendingCompost);
  await fg.run(100);
  check('normal ticks consume the pending compost input', (await compost.state()).world?.find(w => w.name === 'world')?.input?.pending?.total === 0);
  h = await hud(compost);
  check('compost trades that fruit for one dose without selling it', h.plant_food === 1 && h.bag_count === 0 && h.sheckles === cash, [h.plant_food, h.bag_count, h.sheckles]);
  await fg.tap('KeyF'); await fg.run(100);
  check('feeding an empty plot spends nothing', (await hud(compost)).plant_food === 1);
  await fg.tap('KeyE'); await fg.run(100);
  check('a new growing plot enables its Feed button', node(await compost.tree(),'feed')?.props?.disabled === false && (await hud(compost)).feed_ready === true);
  await compost.tap('feed'); await fg.run(100);
  h = await hud(compost);
  const fedPlant = (await plotOf(compost, h)).plant;
  check('feeding spends one dose and names the larger harvest', h.plant_food === 0 && fedPlant?.fed === true
    && h.feeding === 'fed' && !!node(await compost.tree(),'feeding'), [h.plant_food, fedPlant?.fed]);
  if (host !== 'linux') await compost.screenshot(resolve(out,'feeding.png'));
  // Both hosts checkpoint after the HUD's focus input has reached a tick.
  await fg.run(100);
  await fg.save(resolve(out,'compost-growing.world'));
  const finishFood = async session => {
    const g = session.world('world');
    // Opening a native HUD panel queues Blur. Do it before the shared clock
    // steps, so both continuations save after their UI input has been consumed.
    if (!node(await session.tree(),'bag')) await session.tap('bag-tab');
    await g.tap('KeyF'); await g.run(100);
    await g.run(20_100);
    await g.tap('KeyE'); await g.run(100);
    const h = await hud(session), tree = await session.tree();
    const fed = (await g.resources()).Farm?.bag ?? [];
    check('the fed harvest is visibly named in the backpack', fed.length === 1 && fed[0].fed === true && h.bag_count === 1 && h.plant_food === 0
      && tree.nodes.some(n => h.bag[0] && n.props?.text === fruitLabel(h.bag[0].fruit)), [fed, h.bag[0]?.fruit]);
    return g.snapshot();
  };
  const fed = await finishFood(compost);
  await fg.save(resolve(out,'compost-continued.world'));
  pinSave('compost',resolve(out,'compost-continued.world'));
  if (host !== 'linux') await compost.screenshot(resolve(out,'fed-harvest.png'));
  await compost.close();
  const compostBack = await open({fresh:true, world:resolve(out,'compost-growing.world'), epoch:EPOCH});
  await compostBack.tap('play');
  check('compost and fed growth continue identically in a fresh process', JSON.stringify(await finishFood(compostBack)) === JSON.stringify(fed));
  await compostBack.world('world').save(resolve(out,'compost-restored.world'));
  check('fed continuation saves are byte-identical', readFileSync(resolve(out,'compost-continued.world')).equals(readFileSync(resolve(out,'compost-restored.world'))));
  await compostBack.close();
});

// Jev sees the player's text and enabled controls, and acts through those
// controls. It does not inspect the farm, inject money or use the stress tools.
// The harness keeps its own score (orders, harvests, outcome) from published
// values, so the HUD's wording can change without changing when a run ends.
async function playtest({open, out, log}) {
  if (process.argv.includes('--compost')) return compostPlaytest({open, out, log});
  const fullMarket = process.argv.includes('--full-market');
  const startOutside = process.argv.includes('--start-outside');
  if (startOutside && !fullMarket) throw new Error('--start-outside needs --full-market for movement controls');
  const s = await open({epoch:EPOCH});
  await s.tap('play');
  const game = s.world('world');
  await game.run(100);
  if (startOutside) {
    await game.hold('KeyD', 500);
    await game.run(100);
    await game.hold('KeyW', 4_000);
    await game.run(100);
  }
  const transcript = resolve(out, 'jev-decisions.jsonl');
  writeFileSync(transcript, '');
  const crops = fullMarket ? ['carrot','strawberry','blueberry','tomato','corn'] : ['carrot','strawberry','blueberry'];
  const walking = {north:'KeyW', east:'KeyD', south:'KeyS', west:'KeyA'};
  const recent = [];
  let strawberryHarvests = 0;
  // Fruit of one crop in the backpack, from the farm the harness inspects (Jev does not).
  const carried = async id => {
    const kind = (await hud(s)).shop.findIndex(row => row.id === id);
    return ((await game.resources()).Farm?.bag ?? []).filter(item => item.kind === kind).length;
  };
  for (let turn = 0; turn < (fullMarket ? 96 : 48); turn++) {
    const tree = await s.tree();
    const h = await hud(s);
    const state = Object.fromEntries(['sheckles','prompt','plot','planting','held','census','last','weather','restock','objective','order-detail','order-hint','water-count','care','refill-hint']
      .map(id => [id, text(tree, id) ?? '']));
    state.backpack = label(tree, 'bag-tab');
    state.shop = crops.map(id => {
      const row = node(tree, `shop-${id}`);
      const descendants = new Set(row ? [row.id] : []);
      for (const n of tree.nodes) if (descendants.has(n.parent)) descendants.add(n.id);
      return tree.nodes.filter(n => descendants.has(n.id) && n.props?.text).map(n => n.props.text).join(' · ');
    }).filter(Boolean);
    const choices = {wait:'Wait 20 seconds for growth or shop restock'};
    if (fullMarket) for (const direction of Object.keys(walking)) choices[`walk_${direction}`] = `Walk a short distance ${direction}`;
    const buttons = {};
    for (const [id, description] of Object.entries({
      'shop-tab':'Open the seed shop', 'bag-tab':'Open the backpack',
      'equip-order':'Hold the requested seed from your inventory',
      ...Object.fromEntries(crops.flatMap(crop => [[`buy-${crop}`, `Buy one ${crop} seed`], [`equip-${crop}`, `Hold a ${crop} seed`]])),
      'sell-all':`Sell the backpack: ${label(tree, 'sell-all') ?? ''}`,
      deliver:'Deliver the market order for full fruit value plus its bonus',
      water:'Use one watering-can dose on this plot to cut its remaining growth or fruit wait by 25%',
      refill:'Refill all three watering-can doses at the nearby blue barrel',
    })) {
      const n = node(tree, id);
      if (!n || n.props?.disabled || (id === 'shop-tab' && node(tree, 'shop'))
        || (id === 'bag-tab' && node(tree, 'bag')) || (id === 'sell-all' && h.bag_count === 0)) continue;
      const action = id.replaceAll('-', '_');
      choices[action] = description;
      buttons[action] = id;
    }
    if (state.prompt.startsWith('E:')) choices.act = `Press E: ${state.prompt}`;
    const decision = await decide({state:{...state, strawberryHarvests, recent}, choices, transcript,
      goal:(fullMarket ? 'Fill all five market orders: carrot, strawberry, blueberry, tomato, then corn. Walk to an empty tile to plant a different crop; a fruiting plant keeps its tile after harvest. North and east lead into the garden from its starting corner. ' : 'Fill the first two market orders: carrot, then strawberry. Use your current tile. ')
        + 'Deliver requested fruit using the market button to earn its bonus. Buy and plant the requested crop, wait for it to ripen, then harvest with E and deliver. Watering is optional and makes the remaining wait 25% shorter; the blue barrel refills your three doses. Sell only extra fruit not needed for the order. The shop tells you growth times. Avoid buying excess seeds you cannot plant. Waiting advances time without spending money.'});
    log(`JEV ${turn + 1}: ${decision.choice} · ${state.sheckles} · ${state.prompt}`);
    recent.push({action:decision.choice, prompt:state.prompt, purse:state.sheckles});
    if (recent.length > 6) recent.shift();
    if (decision.choice === 'wait') await game.run(20_000);
    else if (decision.choice.startsWith('walk_')) {
      await game.hold(walking[decision.choice.slice(5)], 500);
      await game.run(100);
    } else if (decision.choice === 'act') {
      const before = await carried('strawberry');
      await game.tap('KeyE');
      await game.run(100);
      if (await carried('strawberry') > before) strawberryHarvests++;
    } else {
      await s.tap(buttons[decision.choice]);
      await game.run(100);
    }
    if ((await hud(s)).orders >= (fullMarket ? 5 : 2)) break;
  }
  const tree = await s.tree();
  const h = await hud(s);
  const outcome = {fullMarket, startOutside, strawberryHarvests, purse:h.sheckles, orders:h.orders, census:text(tree,'census'),
    objective:text(tree,'objective'), prompt:text(tree,'prompt'), last:text(tree,'last'), water:text(tree,'water-count'), care:text(tree,'care'), world:await game.snapshot()};
  writeFileSync(resolve(out, 'jev-outcome.json'), JSON.stringify(outcome, null, 2));
  log(`JEV outcome: ${strawberryHarvests} strawberry harvests · ${outcome.purse}¢ · ${outcome.orders} orders`);
  await s.screenshot(resolve(out, 'jev-playtest.png'));
  await s.close();
}

// A new, bounded feature scenario. Earlier market policies stay unchanged.
async function compostPlaytest({open, out, log}) {
  const s = await open({epoch:EPOCH});
  await s.tap('play');
  const game = s.world('world');
  await game.run(100);
  const transcript = resolve(out,'jev-compost-decisions.jsonl');
  writeFileSync(transcript,'');
  const recent = [];
  // The goal is a fed fruit kept in the backpack; the harness reads the farm.
  const fedKept = async () => ((await game.resources()).Farm?.bag ?? []).some(item => item.fed);
  let decisions = 0, achieved = false;
  for (; decisions < 64 && !achieved; decisions++) {
    const tree = await s.tree();
    const state = Object.fromEntries(['sheckles','prompt','held','last','food-count','feeding','water-count','care','refill-hint']
      .map(id => [id,text(tree,id) ?? '']));
    state.backpack = label(tree,'bag-tab');
    const choices = {wait:'Wait 10 seconds for the crop to grow'};
    const buttons = {};
    const offers = {'buy-carrot':'Buy one carrot seed for 10 coins', 'shop-tab':'Open the seed shop',
      'bag-tab':'Open the backpack to sell or compost a chosen fruit',
      feed:'Spend one plant-food dose on this plot for 25% heavier fruit',
      water:'Water this growing plot for a shorter wait', refill:'Refill the watering can at the barrel'};
    for (const n of tree.nodes) if (n.props?.testId?.startsWith('compost-')) offers[n.props.testId] = n.props.accessibilityLabel;
    for (const [id, description] of Object.entries(offers)) {
      const n = node(tree,id);
      if (!n || n.props?.disabled || (id === 'shop-tab' && node(tree,'shop')) || (id === 'bag-tab' && node(tree,'bag'))) continue;
      const action = id.replaceAll('-','_');
      choices[action] = description; buttons[action] = id;
    }
    if (state.prompt.startsWith('E:')) choices.act = `Press E: ${state.prompt}`;
    const decision = await decide({state:{...state,recent},choices,transcript,
      goal:'Grow and harvest a carrot, compost that fruit in the backpack into one plant-food dose, then grow another carrot and feed its plot with F Feed. Harvest the larger Fed fruit and keep it in your backpack. Composting trades away a fruit instead of selling it. You start with one carrot seed and 20 coins; buy a second seed. Stay on this tile. Watering is optional. Wait when growth is the remaining step.'});
    log(`JEV compost ${decisions + 1}: ${decision.choice} · ${state.prompt} · ${state['food-count']}`);
    recent.push({action:decision.choice,prompt:state.prompt,last:state.last});
    if (recent.length > 6) recent.shift();
    if (decision.choice === 'wait') await game.run(10_000);
    else if (decision.choice === 'act') { await game.tap('KeyE'); await game.run(100); }
    else { await s.tap(buttons[decision.choice]); await game.run(100); }
    achieved = await fedKept();
  }
  if (!node(await s.tree(),'bag')) await s.tap('bag-tab');
  const tree = await s.tree();
  const h = await hud(s);
  const outcome = {achieved,decisions,purse:h.sheckles,last:text(tree,'last'),food:h.plant_food,
    backpack:h.bag_count,world:await game.snapshot()};
  writeFileSync(resolve(out,'jev-compost-outcome.json'),JSON.stringify(outcome,null,2));
  log(`JEV compost outcome: achieved=${achieved} · ${decisions} decisions · ${outcome.last}`);
  await s.screenshot(resolve(out,'jev-compost.png'));
  await s.close();
}

// The measurements. Each step reports host wall time for the operation.
async function scale({open, check, out, log}) {
  const s = await open({epoch:EPOCH});
  await s.tap('play');
  const game = s.world('world');
  await s.tap('tools-tab');
  // `perf: true` arms the renderer's sample rings; an unarmed read is zeros.
  const perf = async () => (await s.state('world', undefined, false, false, {world:true, perf:true})).world?.perf;
  const census = h => `${h.plants} plants · ${h.ripe}/${h.fruit} ripe · ${h.mutated} mutated`;
  let last;
  for (const button of ['fill-100', 'fill-1000', 'fill-10000', 'fill-10000']) {
    let t0 = performance.now();
    await s.tap(button);
    await game.run(34);
    const fillMs = ms(t0);
    t0 = performance.now();
    await game.run(60_000);
    const minuteMs = ms(t0);
    t0 = performance.now();
    await s.tree();
    const treeMs = ms(t0);
    const h = await hud(s);
    last = resolve(out, `scale-${h.plants}.world`);
    t0 = performance.now();
    await game.save(last);
    const saveMs = ms(t0);
    const bytes = statSync(last).size;
    const p = await perf();
    log(`SCALE ${census(h)} | fill ${fillMs} ms | +60 s seek ${minuteMs} ms | tree ${treeMs} ms | save ${bytes} B in ${saveMs} ms | perf ${JSON.stringify(p && {tickMs:p.tickMs, frameMs:p.frameMs, draws:p.draws, instances:p.instances, triangles:p.triangles})}`);
    if (button === 'fill-10000' && !process.argv.includes('--huge')) break;
  }
  let t0 = performance.now();
  await game.run(3_600_000);
  log(`SCALE +1 h seek at ${census(await hud(s))}: ${ms(t0)} ms`);
  t0 = performance.now();
  await s.tap('harvest-all');
  await game.run(34);
  log(`SCALE harvest all: ${ms(t0)} ms → ${(await hud(s)).bag_count} fruit`);
  await s.tap('bag-tab');
  t0 = performance.now();
  const bag = await s.tree('bag-list');
  log(`SCALE backpack tree: ${ms(t0)} ms, ${bag.nodes.length} nodes`);
  t0 = performance.now();
  await s.tap('sell-all');
  await game.run(34);
  const sold = await hud(s);
  log(`SCALE sell all: ${ms(t0)} ms → ${sold.sheckles} sheckles`);
  check('sell all pays an exact, finite purse', Number.isFinite(sold.sheckles) && sold.sheckles > 20, sold.sheckles);
  await game.save(resolve(out, 'scale-final.world'));
  await s.close();
  const big = last;
  t0 = performance.now();
  const back = await open({fresh:true, world:big, epoch:EPOCH});
  await back.tap('play');
  await back.world('world').run(34);
  log(`SCALE fresh-process restore of ${statSync(big).size} B: ${ms(t0)} ms → ${census(await hud(back))}`);
  await back.close();
  t0 = performance.now();
  const away = await open({fresh:true, world:big, epoch:EPOCH + 8 * 3_600_000});
  await away.tap('play');
  await away.world('world').run(34);
  log(`SCALE restore 8 h later: ${ms(t0)} ms → ${(await hud(away)).away}`);
  check('the ramp completed', true);
  await away.close();
}
