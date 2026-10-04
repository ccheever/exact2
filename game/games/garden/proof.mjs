#!/usr/bin/env bun
// The garden on a real host: plant, grow, harvest, sell and buy through the
// HUD; a filled garden saved, restored in a fresh process and continued; the
// same save restored an hour later grows offline. `--scale` adds the
// measurements in the diary (entity ramp, long seeks, save sizes).
import {resolve} from 'node:path';
import {readFileSync, statSync, writeFileSync} from 'node:fs';
import { proof, axNames, decide } from '../../proof.mjs';

const EPOCH = Date.parse('2026-10-01T12:00:00Z');
const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
// A text node's label lives on its child text when the button wraps one.
const label = (tree, id) => {
  const n = node(tree, id);
  if (!n) return undefined;
  if (n.props?.text != null) return n.props.text;
  return tree.nodes.find(m => m.parent === n.id && m.props?.text != null)?.props.text;
};
const ms = t => Math.round(performance.now() - t);
// The exact purse, from the sheckles' authored label (its aria-label, the
// runner's accessibilityLabel, on every host). The text itself is compact
// ("379M¢"); a missing node or label is NaN, so every read is checked finite.
const purseOf = tree => Number(node(tree, 'sheckles')?.props?.accessibilityLabel?.split(' ')[0]);

if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave, say}) => {
  const log = say ?? console.log;
  if (process.argv.includes('--playtest')) return playtest({open, out, log});
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
  check('Play is focused and named', node(title, 'play')?.focused === true && (titleAx.unavailable || titleAx.name('play') === 'Play'));
  check('title explains the loop', title.nodes.some(n => n.props?.text === 'It keeps growing while you are away'));
  await s.tap('play');
  const game = s.world('world');
  pin(0, await game.snapshot());
  let t = await s.tree();
  check('starting purse and hand', text(t, 'sheckles') === '20¢' && text(t, 'held') === 'Holding Carrot ×1', [text(t, 'sheckles'), text(t, 'held')]);
  let ax = await axNames(s);
  check('sheckles have an accessible name', ax.unavailable || ax.name('sheckles') === '20 sheckles', ax.name('sheckles'));
  check('the shop lists every seed', (await s.tree('shop-list')).nodes.filter(n => n.props?.testId?.startsWith('shop-')).length === 15);

  await game.run(100);
  check('the prompt offers to plant', text(await s.tree(), 'prompt') === 'E: plant Carrot (1 left)', text(await s.tree(), 'prompt'));
  await game.tap('KeyE');
  await game.run(100);
  t = await s.tree();
  check('E plants the carrot', /^Carrot growing · 0:(19|20)$/.test(text(t, 'prompt')), text(t, 'prompt'));
  check('census counts it', text(t, 'census') === '1 plants · 0/0 ripe · 0 mutated', text(t, 'census'));
  check('held seeds run out', text(t, 'held') === 'No seeds in hand', text(t, 'held'));
  check('planted seeds disappear from shop inventory immediately', !node(t, 'equip-carrot'));
  await game.run(10_000);
  check('the countdown counts down', /^Carrot growing · 0:(09|10)$/.test(text(await s.tree(), 'prompt')), text(await s.tree(), 'prompt'));
  const stage = (await game.snapshot()).entities.find(e => e.components?.Plant)?.components.Plant.stage;
  check('half grown is stage 2', stage === 2, stage);
  await game.run(10_500);
  t = await s.tree();
  check('a ripe carrot can be harvested', text(t, 'prompt') === 'E: harvest 1 Carrot', text(t, 'prompt'));
  await game.tap('KeyE');
  await game.run(100);
  t = await s.tree();
  ax = await axNames(s);
  check('the backpack holds it', label(t, 'bag-tab') === 'Backpack 1' && (ax.unavailable || ax.name('bag-tab') === 'Backpack, 1 fruit'), [label(t, 'bag-tab'), ax.name('bag-tab')]);
  check('a carrot plant is gone after one harvest', text(t, 'census') === '0 plants · 0/0 ripe · 0 mutated', text(t, 'census'));
  await s.tap('bag-tab');
  t = await s.tree();
  ax = await axNames(s);
  check('the backpack lists the fruit', !!node(t, 'bag-0') && !!node(t, 'sell-0') && (ax.unavailable || /^Sell .*Carrot for \d+$/.test(ax.name('sell-0') ?? '')), ax.name('sell-0'));
  await s.tap('sell-all');
  await game.run(100);
  t = await s.tree();
  const purse = purseOf(t);
  check('selling pays', Number.isFinite(purse) && purse > 20 && label(t, 'bag-tab') === 'Backpack 0', [purse, label(t, 'bag-tab')]);
  check('an empty backpack says how to fill it', !!node(t, 'bag-empty'));

  await s.tap('shop-tab');
  await s.tap('buy-carrot');
  await game.run(100);
  t = await s.tree();
  check('buying spends and fills the hand', purseOf(t) === purse - 10 && text(t, 'held') === 'Holding Carrot ×1', [purseOf(t), text(t, 'held')]);
  ax = await axNames(s);
  check('an owned seed offers to hold it', !!node(t, 'equip-carrot') && (ax.unavailable || ax.name('equip-carrot') === 'Hold Carrot, 1 owned'), ax.name('equip-carrot'));
  check('an unaffordable seed is disabled', node(t, 'buy-grape')?.props?.disabled === true, node(t, 'buy-grape')?.props);
  // Two presses with no tick between them are two messages.
  await s.tap('buy-carrot');
  await s.tap('buy-carrot');
  await game.run(100);
  const held = text(await s.tree(), 'held');
  check('two presses between ticks buy two', held === 'Holding Carrot ×3', held);

  await s.tap('tools-tab');
  await s.tap('fill-1000');
  await game.run(100);
  t = await s.tree();
  check('fill plants a thousand', text(t, 'census')?.startsWith('1000 plants'), text(t, 'census'));
  await game.run(150_000);
  t = await s.tree();
  const census = text(t, 'census');
  check('two and a half minutes ripen fruit', /^\d+ plants · [1-9]\d*\/\d+ ripe/.test(census), census);
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
  t = await later.tree();
  check('an hour later the garden grew while away', /^While you were away \(\d+:\d\d\): \d+ events/.test(text(t, 'away') ?? ''), text(t, 'away'));
  const ripe = Number(text(t, 'census')?.match(/· (\d+)\//)?.[1]);
  check('and every fruit is ripe', ripe > Number(census.match(/· (\d+)\//)[1]), [census, text(t, 'census')]);
  await later.tap('tools-tab');
  await later.tap('harvest-all');
  await grown.run(100);
  t = await later.tree();
  check('harvest all fills the backpack', Number(label(t, 'bag-tab')?.split(' ')[1]) === ripe, [label(t, 'bag-tab'), ripe]);
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
  check('the market names its first crop', text(await market.tree(), 'objective') === 'Market order 1 · 1 Carrot');
  check('an unfilled order is disabled', node(await market.tree(), 'deliver')?.props?.disabled === true);
  await mg.tap('KeyE');
  await mg.run(20_100);
  await mg.tap('KeyE');
  await mg.run(100);
  await mg.save(resolve(out, 'market.world'));
  const deliverAndGrow = async session => {
    const g = session.world('world');
    const before = purseOf(await session.tree());
    await session.tap('deliver');
    await g.run(100);
    let tree = await session.tree();
    check('delivery advances the order and pays fruit plus bonus', text(tree, 'objective') === 'Market order 2 · 4 Strawberry' && purseOf(tree) > before + 30);
    check('bonus immediately enables strawberry purchase', node(tree, 'buy-strawberry')?.props?.disabled === false);
    await session.tap('buy-strawberry');
    await g.run(100);
    check('purchase feedback names the seed', text(await session.tree(), 'last') === 'Bought Strawberry seed');
    await g.tap('KeyE');
    await g.run(70_100);
    await g.tap('KeyE');
    await g.run(100);
    await session.tap('deliver');
    await g.run(100);
    tree = await session.tree();
    check('the second delivery funds blueberry', text(tree, 'objective') === 'Market order 3 · 5 Blueberry' && purseOf(tree) >= 400 && node(tree,'buy-blueberry')?.props?.disabled === false);
    check('delivered fruit leaves the backpack', label(tree, 'bag-tab') === 'Backpack 0');
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
});

// Jev sees the player's text and enabled controls, and acts through those
// controls. It does not inspect the farm, inject money or use the stress tools.
async function playtest({open, out, log}) {
  const s = await open({epoch:EPOCH});
  await s.tap('play');
  const game = s.world('world');
  await game.run(100);
  const transcript = resolve(out, 'jev-decisions.jsonl');
  writeFileSync(transcript, '');
  const recent = [];
  let strawberryHarvests = 0;
  for (let turn = 0; turn < 48; turn++) {
    const tree = await s.tree();
    const state = Object.fromEntries(['sheckles','prompt','held','census','last','weather','restock','objective','order-detail']
      .map(id => [id, text(tree, id) ?? '']));
    state.backpack = label(tree, 'bag-tab');
    state.shop = ['carrot','strawberry','blueberry'].map(id => {
      const row = node(tree, `shop-${id}`);
      const descendants = new Set(row ? [row.id] : []);
      for (const n of tree.nodes) if (descendants.has(n.parent)) descendants.add(n.id);
      return tree.nodes.filter(n => descendants.has(n.id) && n.props?.text).map(n => n.props.text).join(' · ');
    }).filter(Boolean);
    const choices = {wait:'Wait 20 seconds for growth or shop restock'};
    const buttons = {};
    for (const [id, description] of Object.entries({
      'shop-tab':'Open the seed shop', 'bag-tab':'Open the backpack',
      'buy-carrot':'Buy one carrot seed', 'buy-strawberry':'Buy one strawberry seed',
      'buy-blueberry':'Buy one blueberry seed', 'equip-carrot':'Hold a carrot seed',
      'equip-strawberry':'Hold a strawberry seed', 'equip-blueberry':'Hold a blueberry seed',
      'sell-all':`Sell the backpack: ${label(tree, 'sell-all') ?? ''}`,
      deliver:'Deliver the market order for full fruit value plus its bonus',
    })) {
      const n = node(tree, id);
      if (!n || n.props?.disabled || (id === 'shop-tab' && node(tree, 'shop'))
        || (id === 'bag-tab' && node(tree, 'bag')) || (id === 'sell-all' && state.backpack === 'Backpack 0')) continue;
      const action = id.replaceAll('-', '_');
      choices[action] = description;
      buttons[action] = id;
    }
    if (state.prompt.startsWith('E:')) choices.act = `Press E: ${state.prompt}`;
    const decision = await decide({state:{...state, strawberryHarvests, recent}, choices, transcript,
      goal:'Fill the first two market orders: carrot, then strawberry. Deliver requested fruit using the market button to earn its bonus. Buy and plant the requested crop, wait for it to ripen, then harvest with E and deliver. Use your current tile. Sell only extra fruit not needed for the order. The shop tells you growth times. Avoid buying excess seeds you cannot plant. Waiting advances time without spending money.'});
    log(`JEV ${turn + 1}: ${decision.choice} · ${state.sheckles} · ${state.prompt}`);
    recent.push({action:decision.choice, prompt:state.prompt, purse:state.sheckles});
    if (recent.length > 6) recent.shift();
    if (decision.choice === 'wait') await game.run(20_000);
    else if (decision.choice === 'act') {
      if (/^E: harvest \d+ Strawberry$/.test(state.prompt)) strawberryHarvests++;
      await game.tap('KeyE');
      await game.run(100);
    } else {
      await s.tap(buttons[decision.choice]);
      await game.run(100);
    }
    if (text(await s.tree(), 'objective')?.startsWith('Market order 3')) break;
  }
  const tree = await s.tree();
  const outcome = {strawberryHarvests, purse:purseOf(tree), census:text(tree,'census'),
    objective:text(tree,'objective'), prompt:text(tree,'prompt'), last:text(tree,'last'), world:await game.snapshot()};
  writeFileSync(resolve(out, 'jev-outcome.json'), JSON.stringify(outcome, null, 2));
  log(`JEV outcome: ${strawberryHarvests} strawberry harvests · ${outcome.purse}¢ · ${outcome.census}`);
  await s.screenshot(resolve(out, 'jev-playtest.png'));
  await s.close();
}

// The measurements. Each step reports host wall time for the operation.
async function scale({open, check, out, log}) {
  const s = await open({epoch:EPOCH});
  await s.tap('play');
  const game = s.world('world');
  await s.tap('tools-tab');
  const perf = async () => (await s.state('world')).world?.perf ?? (await s.state('world')).perf;
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
    const tree = await s.tree();
    const treeMs = ms(t0);
    const census = text(tree, 'census');
    const plants = census.split(' ')[0];
    last = resolve(out, `scale-${plants}.world`);
    t0 = performance.now();
    await game.save(last);
    const saveMs = ms(t0);
    const bytes = statSync(last).size;
    const p = await perf();
    log(`SCALE ${census} | fill ${fillMs} ms | +60 s seek ${minuteMs} ms | tree ${treeMs} ms | save ${bytes} B in ${saveMs} ms | perf ${JSON.stringify(p && {tickMs:p.tickMs, frameMs:p.frameMs, draws:p.draws, instances:p.instances, triangles:p.triangles})}`);
    if (button === 'fill-10000' && !process.argv.includes('--huge')) break;
  }
  let t0 = performance.now();
  await game.run(3_600_000);
  log(`SCALE +1 h seek at ${text(await s.tree(), 'census')}: ${ms(t0)} ms`);
  t0 = performance.now();
  await s.tap('harvest-all');
  await game.run(34);
  log(`SCALE harvest all: ${ms(t0)} ms → ${label(await s.tree(), 'bag-tab')}`);
  await s.tap('bag-tab');
  t0 = performance.now();
  const bag = await s.tree('bag-list');
  log(`SCALE backpack tree: ${ms(t0)} ms, ${bag.nodes.length} nodes`);
  t0 = performance.now();
  await s.tap('sell-all');
  await game.run(34);
  const sold = await s.tree();
  log(`SCALE sell all: ${ms(t0)} ms → ${purseOf(sold)} sheckles (${text(sold, 'sheckles')})`);
  check('sell all pays an exact, finite purse', Number.isFinite(purseOf(sold)) && purseOf(sold) > 20, node(sold, 'sheckles'));
  await game.save(resolve(out, 'scale-final.world'));
  await s.close();
  const big = last;
  t0 = performance.now();
  const back = await open({fresh:true, world:big, epoch:EPOCH});
  await back.tap('play');
  await back.world('world').run(34);
  log(`SCALE fresh-process restore of ${statSync(big).size} B: ${ms(t0)} ms → ${text(await back.tree(), 'census')}`);
  await back.close();
  t0 = performance.now();
  const away = await open({fresh:true, world:big, epoch:EPOCH + 8 * 3_600_000});
  await away.tap('play');
  await away.world('world').run(34);
  log(`SCALE restore 8 h later: ${ms(t0)} ms → ${text(await away.tree(), 'away')}`);
  check('the ramp completed', true);
  await away.close();
}
