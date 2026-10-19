import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {existsSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {drawnShare, proofGame} from './proof.mjs';
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

// The template's first-frame fact: a lit scene passes, a cleared canvas under a HUD fails.
test('drawnShare tells a drawn world from a cleared canvas under a HUD', () => {
  const [width, height] = [320, 180];
  const image = paint => {
    const data = new Uint8Array(width * height * 4);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) data.set([...paint(x, y), 255], (y * width + x) * 4);
    return encodePng({width, height, data});
  };
  const clear = [126, 171, 163];
  // Four HUD pills, about 4% of the frame, over the clear colour.
  const hud = (x, y) => [[8, 8], [270, 8], [8, 140], [270, 140]].some(([px, py]) => x >= px && x < px + 42 && y >= py && y < py + 14);
  expect(drawnShare(image((x, y) => hud(x, y) ? [32, 39, 49] : clear))).toBeLessThan(0.1);
  // A ground plane shaded towards the horizon fills the lower two thirds.
  expect(drawnShare(image((x, y) => y > 60 ? [90, 120 + (y >> 2), 115] : clear))).toBeGreaterThan(0.5);
});

test('a proof script run from / names its commands by absolute path', () => {
  const said = spawnSync(process.execPath, ['-e', `import {proofCommand} from ${JSON.stringify(resolve(import.meta.dir, 'proof.mjs'))}; console.log(proofCommand(${JSON.stringify(resolve(import.meta.dir, 'prove.mjs'))}, 'reef'))`], {cwd:'/', encoding:'utf8'});
  expect(said.stdout.trim()).toBe(`bun ${resolve(import.meta.dir, 'prove.mjs')} reef`);
});
