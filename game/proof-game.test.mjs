import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {existsSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {proofGame, worldPixels} from './proof.mjs';
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

// The template's completed-frame fact: a blank world under a large HUD fails, a sparse
// scene passes, and the screenshot's scale maps layout points to pixels.
test('worldPixels tells a drawn world from a cleared canvas, whatever covers it', () => {
  const [width, height, scale] = [640, 360, 2];
  const clear = [126, 171, 163], hudColour = [32, 39, 49];
  // A fifth of the canvas under HUD panels, in layout points.
  const hud = [{x:0, y:0, w:320, h:20}, {x:0, y:160, w:320, h:20}, {x:0, y:20, w:32, h:140}];
  const layout = {viewport:{w:320, h:180}, nodes:[{id:1, x:0, y:0, w:320, h:180, type:'View'},
    {id:2, x:0, y:0, w:320, h:180, type:'Canvas', testId:'world'}, ...hud.map((b, i) => ({id:3 + i, ...b, type:'View'}))]};
  const covered = (x, y) => hud.some(b => x / scale >= b.x && x / scale < b.x + b.w && y / scale >= b.y && y / scale < b.y + b.h);
  const image = paint => {
    const data = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) data.set([...(covered(x, y) ? hudColour : paint(x, y)), 255], (y * width + x) * 4);
    return encodePng({width, height, data});
  };
  const shot = {w:320, h:180};
  const blank = worldPixels(image(() => clear), shot, layout);
  expect(blank.hud).toBe(3);
  expect(blank.drawn).toBe(0);
  // One small object: a hundredth of the open canvas.
  const sparse = worldPixels(image((x, y) => x >= 300 && x < 340 && y >= 160 && y < 200 ? [200, 90, 40] : clear), shot, layout);
  expect(sparse.drawn).toBe(1600);
  expect(sparse.share).toBeGreaterThan(0.005);
  expect(sparse.share).toBeLessThan(0.02);
  expect(() => worldPixels(image(() => clear), shot, {...layout, nodes:layout.nodes.slice(0, 1)})).toThrow('no canvas world');
});

test('a proof script run from / names its commands by absolute path', () => {
  const said = spawnSync(process.execPath, ['-e', `import {proofCommand} from ${JSON.stringify(resolve(import.meta.dir, 'proof.mjs'))}; console.log(proofCommand(${JSON.stringify(resolve(import.meta.dir, 'prove.mjs'))}, 'reef'))`], {cwd:'/', encoding:'utf8'});
  expect(said.stdout.trim()).toBe(`bun ${resolve(import.meta.dir, 'prove.mjs')} reef`);
});
