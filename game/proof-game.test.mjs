import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, realpathSync, rmSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {tmpdir} from 'node:os';
import {proofGame} from './proof.mjs';

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
