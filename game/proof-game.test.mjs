import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {existsSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {completedWorldFrame, proofGame, worldPixels} from './proof.mjs';
import {encodePng} from '../scripts/png.mjs';

// LLP 1132 D7: a proof script need not sit at the game's root.
test('a proof script finds the game above it, or the one --app names', () => {
  const root = realpathSync(mkdtempSync(resolve(tmpdir(), 'proof-game-')));
  const game = resolve(root, 'reef'), tools = resolve(game, 'tools/playtest'), outside = resolve(root, 'scripts');
  try {
    mkdirSync(resolve(game, 'logic/src'), {recursive:true});
    writeFileSync(resolve(game, 'logic/src/lib.rs'), '');
    mkdirSync(tools, {recursive:true});
    mkdirSync(outside);
    expect(proofGame(game, ['web'])).toBe(game);
    expect(proofGame(tools, ['linux'])).toBe(game);
    expect(proofGame(outside, ['web', '--app', game])).toBe(game);
    expect(() => proofGame(outside, ['web'])).toThrow(/no game at or above this script[^]*--app <game directory>/);
    expect(() => proofGame(game, ['--app', outside])).toThrow(/not a game/);
    expect(() => proofGame(game, ['--app'])).toThrow(/--app requires/);
    // app.json alone marks a game whose logic lives elsewhere (game.crate in app.json).
    const declared = resolve(root, 'declared');
    mkdirSync(resolve(declared, 'tools'), {recursive:true});
    writeFileSync(resolve(declared, 'app.json'), '{}');
    expect(proofGame(resolve(declared, 'tools'), [])).toBe(declared);
  } finally { rmSync(root, {recursive:true, force:true}); }
});

// proof() itself, not only the walker: a script under tools/ writes its run into the
// game's artifacts and names the game. An unknown host stops it before any build.
test('a proof script under tools/ runs proof() against the game around it', () => {
  const root = realpathSync(mkdtempSync(resolve(tmpdir(), 'proof-tools-')));
  const game = resolve(root, 'lagoon'), tools = resolve(game, 'tools');
  try {
    mkdirSync(resolve(game, 'logic/src'), {recursive:true});
    mkdirSync(tools);
    writeFileSync(resolve(game, 'logic/src/lib.rs'), '');
    writeFileSync(resolve(game, 'pins.json'), JSON.stringify({ticks:{}, saves:{}}));
    writeFileSync(resolve(tools, 'playtest.mjs'), `import {proof} from ${JSON.stringify(resolve(import.meta.dir, 'proof.mjs'))};
await proof(import.meta, async () => { throw new Error('the script never runs for an unknown host'); });\n`);
    const run = spawnSync(process.execPath, [resolve(tools, 'playtest.mjs'), 'no-such-host'], {cwd:root, encoding:'utf8'});
    expect(run.status).toBe(1);
    expect(run.stdout).toContain('proof host unavailable: no-such-host');
    expect(existsSync(resolve(tools, 'artifacts'))).toBe(false);
    const summary = JSON.parse(readFileSync(resolve(game, 'artifacts/no-such-host/summary.json'), 'utf8'));
    expect(summary.name).toBe('lagoon');
  } finally { rmSync(root, {recursive:true, force:true}); }
});

// The template's completed-frame fact. The HUD is what the canvas's subtree draws, by
// ancestry in the tree, whatever its size; a sibling or ancestor is not HUD.
test('worldPixels masks the canvas subtree, so a cleared world fails and a sparse one passes', () => {
  const [width, height, scale] = [640, 360, 2], shot = {w:320, h:180};
  const clear = [126, 171, 163], ink = [32, 39, 49];
  // Nodes: 1 root; 2 the canvas; 3–5 its HUD children; 6 a sibling of the canvas.
  const scene = ({hud = [], sibling = null, paint}) => {
    const boxes = [{id:1, x:0, y:0, w:320, h:180, type:'View'}, {id:2, x:0, y:0, w:320, h:180, type:'Canvas', testId:'world'},
      ...hud.map((b, i) => ({id:3 + i, ...b, type:'View'})), ...(sibling ? [{id:6, ...sibling, type:'View'}] : [])];
    const tree = {nodes:[{id:1, parent:null, props:{}}, {id:2, parent:1, props:{testId:'world'}},
      ...hud.map((_, i) => ({id:3 + i, parent:2, props:{}})), ...(sibling ? [{id:6, parent:1, props:{}}] : [])]};
    const onHud = (x, y) => hud.some(b => x / scale >= b.x && x / scale < b.x + b.w && y / scale >= b.y && y / scale < b.y + b.h);
    const data = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) data.set([...(onHud(x, y) ? ink : paint(x, y)), 255], (y * width + x) * 4);
    return worldPixels(encodePng({width, height, data}), shot, {viewport:{w:320, h:180}, nodes:boxes}, tree);
  };
  const cleared = () => clear;
  const sparse = (x, y) => x >= 300 && x < 340 && y >= 160 && y < 200 ? [200, 90, 40] : clear;
  const bars = [{x:0, y:0, w:320, h:20}, {x:0, y:160, w:320, h:20}, {x:0, y:20, w:32, h:140}];
  const ready = {ready:true, perf:{firstFrameMs:12}};
  // A fifth of the canvas under HUD bars: cleared fails, a hundredth drawn passes.
  expect(completedWorldFrame(scene({hud:bars, paint:cleared}), ready).ok).toBe(false);
  const few = scene({hud:bars, paint:sparse});
  expect([few.drawn, completedWorldFrame(few, ready).ok]).toEqual([1600, true]);
  // HUD that overflows the canvas is masked inside it, and only there.
  const overflow = [{x:-40, y:-40, w:120, h:80}, {x:280, y:140, w:200, h:200}];
  expect(scene({hud:overflow, paint:cleared}).drawn).toBe(0);
  expect(completedWorldFrame(scene({hud:overflow, paint:sparse}), ready).ok).toBe(true);
  // A painted panel over the whole canvas leaves no world pixel: the fact rests on the
  // renderer's reports and says so, so a world that never drew fails it.
  const covered = scene({hud:[{x:0, y:0, w:320, h:180}], paint:cleared});
  expect(covered.pixels).toBe(0);
  expect(completedWorldFrame(covered, {ready:false})).toMatchObject({ok:false, fallback:true});
  expect(completedWorldFrame(covered, {ready:true, perf:{firstFrameMs:null}}).ok).toBe(false);
  expect(completedWorldFrame(covered, ready)).toMatchObject({ok:true, fallback:true});
  // A sibling under the canvas's box is not HUD: a valid world shows through and passes.
  const behind = scene({sibling:{x:0, y:0, w:160, h:180}, paint:sparse});
  expect([behind.hud, behind.pixels, completedWorldFrame(behind, ready).ok]).toEqual([0, width * height, true]);
  expect(() => scene({paint:cleared, hud:[]}) && worldPixels(encodePng({width:2, height:2, data:new Uint8Array(16)}), shot, {viewport:{w:320, h:180}, nodes:[]}, {nodes:[]})).toThrow('no canvas world');
});

test('a proof script run from / names its commands by absolute path', () => {
  const said = spawnSync(process.execPath, ['-e', `import {proofCommand} from ${JSON.stringify(resolve(import.meta.dir, 'proof.mjs'))}; console.log(proofCommand(${JSON.stringify(resolve(import.meta.dir, 'prove.mjs'))}, 'reef'))`], {cwd:'/', encoding:'utf8'});
  expect(said.stdout.trim()).toBe(`bun ${resolve(import.meta.dir, 'prove.mjs')} reef`);
});
