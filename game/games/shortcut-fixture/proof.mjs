#!/usr/bin/env bun
// One shortcut rule on every host (LLP 1132 D11): with the world's canvas
// focused, a key a shown button's `aria-keyshortcuts` declares presses that
// button once (a repeat presses nothing, a disabled button nothing), never
// reaches the world and moves no focus, and nothing is left held.
// `bun game/games/shortcut-fixture/proof.mjs <linux|web|macos> [--paranoid]`.
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';

if (import.meta.main) await proof(import.meta, async ({open, check, out, pin, pinSave}) => {
  const shop = async w => (await w.resources()).Shop;
  const input = async s => (await s.state()).world[0].input;
  const settled = (i) => ['held', 'forwarded', 'controls', 'forwardedControls', 'controlContacts'].every(k => i[k].length === 0);
  const s = await open(), w = s.world('world');
  await w.run(100);
  await w.tap('Digit1');
  await w.run(100);
  let got = await shop(w);
  check('Digit1 presses the Buy button once', got.buys === 1, got);
  check('the focused world does not hear Digit1', got.ones === 0, got);
  let held = await input(s);
  check('nothing is held after the tap', settled(held), held);
  await w.key_down('Digit1');
  await w.run(200);
  got = await shop(w);
  check('a held Digit1 presses Buy once more and still not the world', got.buys === 2 && got.ones === 0, got);
  await w.key_up('Digit1');
  await w.run(100);
  held = await input(s);
  check('the key-up leaves nothing held', settled(held), held);
  check('the HUD counts two purchases', (await s.tree()).nodes.find(n => n.props?.testId === 'bought')?.props?.text === 'Bought 2');
  const snapshot = await w.snapshot();
  pin(snapshot.tick, snapshot);
  const saved = resolve(out, 'shortcut.world');
  await w.save(saved);
  pinSave('after-shortcuts', saved);
  // The press moves no focus: a held key stays the world's, and Space after it is the world's.
  await w.key_down('KeyW');
  await w.run(100);
  await w.tap('Digit1');
  await w.run(100);
  got = await shop(w);
  held = await input(s);
  check('with W held, Digit1 buys once more', got.buys === 3 && got.ones === 0, got);
  check('the world still holds W after the shortcut', held.held.includes('KeyW'), held);
  check('the canvas keeps the focus', (await s.tree()).nodes.find(n => n.props?.testId === 'world')?.focused === true);
  await w.key_up('KeyW');
  await w.tap('Space');
  await w.run(100);
  got = await shop(w);
  check('Space after the shortcut is the world\'s jump, not Buy', got.jumps === 1 && got.buys === 3, got);
  // A repeat is not a press: Digit1 held past the 500 ms repeat delay buys once.
  await w.hold('Digit1', 800);
  await w.run(100);
  got = await shop(w);
  check('a held, repeating Digit1 buys once', got.buys === 4 && got.ones === 0, got);
  held = await input(s);
  check('nothing is held after the repeats', settled(held), held);
  // A disabled Buy still owns its key, as on the web: nothing is bought and the world does not hear it.
  await s.tap('lock');
  await w.tap('Digit1');
  await w.run(100);
  got = await shop(w);
  check('a disabled Buy takes Digit1 and buys nothing', got.buys === 4 && got.ones === 0, got);
  await s.close();
  const r = await open({world:saved}), rw = r.world('world');
  await rw.run(100);
  await rw.tap('Digit1');
  await rw.run(100);
  got = await shop(rw);
  check('restored, Digit1 presses Buy once and not the world', got.buys === 3 && got.ones === 0, got);
  held = await input(r);
  check('restored, nothing is held after the tap', settled(held), held);
  await r.close();
});
