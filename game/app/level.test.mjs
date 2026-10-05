import {test, expect} from 'bun:test';
import {mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, realpathSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {gameDefaults, gameShells} from './shells.mjs';

test('external shells use the SDK toolchain and refresh stale generated pins', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-shell-toolchain-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    const authored = '[toolchain]\nchannel = "author-owned"\n';
    writeFileSync(resolve(dir, 'rust-toolchain.toml'), authored);
    const game = gameDefaults(dir).game;
    const generate = () => gameShells(dir, game, resolve(import.meta.dir, '..'));
    const generated = resolve(dir, '.shells/rust-toolchain.toml');
    const pin = readFileSync(resolve(import.meta.dir, '../../rust-toolchain.toml'), 'utf8');
    generate();
    expect(readFileSync(generated, 'utf8')).toBe(pin);
    writeFileSync(generated, '[toolchain]\nchannel = "stale"\n');
    generate();
    expect(readFileSync(generated, 'utf8')).toBe(pin);
    expect(readFileSync(resolve(dir, 'rust-toolchain.toml'), 'utf8')).toBe(authored);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('a level-only game bakes its declared type without an art directory', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-level-shell-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{}}));
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    const game = gameDefaults(dir).game;
    gameShells(dir, game, resolve(import.meta.dir, '..'));
    const build = readFileSync(resolve(dir, '.shells/gpu/build.rs'), 'utf8');
    expect(build).toContain('type App = game_logic::Island;');
    expect(build).toContain('bake_files::bake_game_level::<App>');
    expect(build.replaceAll('\\\\', '/')).toContain('bake/src/files.rs');
    expect(build).not.toContain('bake_art(');
    expect(readFileSync(resolve(dir, '.shells/gpu/Cargo.toml'), 'utf8')).not.toContain('exact-game-bake.workspace = true');
    expect(readFileSync(resolve(dir, '.shells/web/Cargo.toml'), 'utf8')).not.toContain('exact-data-host');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('a declared app data crate is linked into generated game hosts', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-data-shell-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    mkdirSync(resolve(dir, 'data/src'), {recursive:true});
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{data:{crate:'island-data',type:'SaveData'}}}));
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "island-data"\nversion = "0.1.0"\nedition = "2021"\nworkspace = "../.shells"\n');
    const game = gameDefaults(dir).game;
    gameShells(dir, game, resolve(import.meta.dir, '..'));
    const workspace = readFileSync(resolve(dir, '.shells/Cargo.toml'), 'utf8');
    const web = readFileSync(resolve(dir, '.shells/web/Cargo.toml'), 'utf8');
    const linux = readFileSync(resolve(dir, '.shells/linux/Cargo.toml'), 'utf8');
    expect(workspace).toContain('"../data"');
    for (const manifest of [web, linux]) {
      expect(manifest).toContain('exact-data-host.workspace = true');
      expect(manifest.match(/app-data = /g)).toHaveLength(2);
    }
    for (const host of ['web', 'apple', 'linux']) {
      expect(readFileSync(resolve(dir, `.shells/${host}/build.rs`), 'utf8'))
        .toContain('bake_data_declaration::<app_data::SaveData>');
    }
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "other-data"\nversion = "0.1.0"\nworkspace = "../.shells"\n');
    expect(() => gameShells(dir, game, resolve(import.meta.dir, '..')))
      .toThrow('game.data.crate island-data must name the package');
    writeFileSync(resolve(dir, 'data/Cargo.toml'), '[package]\nname = "island-data"\nversion = "0.1.0"\nworkspace = ".."\n');
    expect(() => gameShells(dir, game, resolve(import.meta.dir, '..')))
      .toThrow('set package.workspace = "../.shells"');
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('render declarations fail before creating or changing generated files', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-render-declaration-'));
  const valid = {crate:'island-render', hooks:'view::Hooks', shaders:'shaders::REGISTRY'};
  const write = value => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{render:value}}));
  const shells = () => gameShells(dir, gameDefaults(dir).game, resolve(import.meta.dir, '..'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    for (const invalid of [null, [], {}, {...valid, crate:'island'}, {...valid, hooks:'x()'}, {...valid, shaders:'&[]'}, {...valid, shaders:null}, {...valid, extra:true}]) {
      write(invalid); expect(shells).toThrow('game.render'); expect(existsSync(resolve(dir, '.shells'))).toBe(false);
    }
    write(valid); expect(shells).toThrow('requires'); expect(existsSync(resolve(dir, '.shells'))).toBe(false);
    mkdirSync(resolve(dir, 'render/src'), {recursive:true});
    const manifest = resolve(dir, 'render/Cargo.toml');
    writeFileSync(manifest, '[package]\nname = "wrong-render"\nworkspace = "../.shells"\n');
    expect(shells).toThrow('must name the package'); expect(existsSync(resolve(dir, '.shells'))).toBe(false);
    writeFileSync(manifest, '[package]\nname = "island-render"\nworkspace = ".."\n');
    expect(shells).toThrow('package.workspace'); expect(existsSync(resolve(dir, '.shells'))).toBe(false);
    writeFileSync(manifest, '[package]\nname = "island-render"\nworkspace = "../.shells"\n');
    shells(); const before = readFileSync(resolve(dir, '.shells/Cargo.toml'), 'utf8');
    write({...valid, shaders:'call()'}); expect(shells).toThrow('game.render');
    expect(readFileSync(resolve(dir, '.shells/Cargo.toml'), 'utf8')).toBe(before);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('render hooks compose with audio/assets and never enter generated host or build dependencies', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-render-shell-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true}); mkdirSync(resolve(dir, 'render/src'), {recursive:true});
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'render/Cargo.toml'), '[package]\nname = "island-render"\nworkspace = "../.shells"\n');
    const generate = game => {
      writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game}));
      gameShells(dir, gameDefaults(dir).game, resolve(import.meta.dir, '..'));
      return readFileSync(resolve(dir, '.shells/gpu/src/lib.rs'), 'utf8');
    };
    for (const audio of [false,true]) for (const assets of [false,true]) {
      const plain = generate({audio,assets});
      const manifests = Object.fromEntries(['gpu','web','apple','linux','windows'].map(kind => [kind, readFileSync(resolve(dir, `.shells/${kind}/Cargo.toml`), 'utf8')]));
      for (const shaders of [undefined,'shaders::REGISTRY']) {
        const hooked = generate({audio,assets,render:{crate:'island-render',hooks:'view::Hooks',...(shaders ? {shaders} : {})}});
        expect(hooked).toBe(plain.replace(');', `, hooks = game_render::view::Hooks${shaders ? ', shaders = game_render::shaders::REGISTRY' : ''});`));
        const gpu = readFileSync(resolve(dir, '.shells/gpu/Cargo.toml'), 'utf8');
        expect(gpu.split('[build-dependencies]')[1]).not.toContain('game-render');
        expect(gpu.match(/game-render = /g)).toHaveLength(1);
        for (const kind of ['web','apple','linux','windows']) expect(readFileSync(resolve(dir, `.shells/${kind}/Cargo.toml`), 'utf8')).toBe(manifests[kind]);
        expect(readFileSync(resolve(dir, '.shells/Cargo.toml'), 'utf8')).toContain('../render');
      }
      expect(generate({audio,assets})).toBe(plain);
      expect(readFileSync(resolve(dir, '.shells/gpu/Cargo.toml'), 'utf8')).toBe(manifests.gpu);
    }
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('render crate source and shader changes invalidate GPU proof inputs without invalidating the host', async () => {
  const {proofInputs, proofInputExcluded} = await import('../proof.mjs');
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-render-inputs-')), app = resolve(dir, 'game/games/island');
  const write = (file, value) => { const path = resolve(app,file); mkdirSync(resolve(path,'..'),{recursive:true}); writeFileSync(path,value); };
  try {
    write('app.contract','component Island\n  view\n'); write('logic/src/lib.rs','logic');
    write('render/Cargo.toml','[package]\nname="island-render"\n');
    write('render/src/lib.rs','hooks'); write('render/shaders/fog.wgsl','shader');
    const read = () => proofInputs(dir, app, resolve(dir, 'cache'));
    for (const file of ['render/Cargo.toml','render/src/lib.rs','render/shaders/fog.wgsl','render/build.mjs']) {
      expect(proofInputExcluded(`game/games/island/${file}`,'island')).toBe(false);
      const before=read(); write(file,'changed'); const after=read();
      expect(after.gpu).not.toBe(before.gpu); expect(after.host).toBe(before.host);
    }
  } finally { rmSync(dir,{recursive:true,force:true}); }
});

test('the old game.presentation key fails with its rename', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-render-rename-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true});
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{presentation:{crate:'island-presentation', type:'Hooks'}}}));
    expect(() => gameShells(dir, gameDefaults(dir).game, resolve(import.meta.dir, '..')))
      .toThrow('game.presentation is now game.render');
    expect(existsSync(resolve(dir, '.shells'))).toBe(false);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});

test('the bake writes the render crate its assembled shader inventory, engine preludes included', async () => {
  const {ENGINE_PRELUDES} = await import('../../scripts/app.mjs');
  // The sets are the renderer's own constants, file for file.
  const materials = readFileSync(resolve(import.meta.dir, '../render/src/hooks/materials.rs'), 'utf8');
  const constant = name => [...materials.slice(materials.indexOf(`pub const ${name}`)).split(');')[0].matchAll(/shaders\/(\w+\.wgsl)/g)].map(m => m[1]);
  expect(ENGINE_PRELUDES['exact-game-render:material']).toEqual(constant('MATERIAL_WGSL'));
  expect(ENGINE_PRELUDES['exact-game-render:material_shadows']).toEqual(constant('MATERIAL_SHADOWS_WGSL'));
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-render-inventory-'));
  try {
    mkdirSync(resolve(dir, 'logic/src'), {recursive:true}); mkdirSync(resolve(dir, 'render/shaders'), {recursive:true});
    writeFileSync(resolve(dir, 'logic/src/lib.rs'), 'impl Game for Island { const ID: &\'static str = "island"; }');
    writeFileSync(resolve(dir, 'render/Cargo.toml'), '[package]\nname = "island-render"\nworkspace = "../.shells"\n');
    writeFileSync(resolve(dir, 'render/shaders/sky.wgsl'), '// sky body\n');
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({game:{render:{crate:'island-render', hooks:'Hooks'}},
      gpu:{shaderRoots:['render/shaders'], shaderPreludes:{sky:['exact-game-render:frame']}}}));
    gameShells(dir, gameDefaults(dir).game, resolve(import.meta.dir, '..'));
    const frame = readFileSync(resolve(import.meta.dir, '../render/src/shaders/frame.wgsl'), 'utf8');
    expect(readFileSync(resolve(dir, '.shells/shaders/sky.wgsl'), 'utf8')).toBe(`${frame}\n// sky body\n`);
    expect(readFileSync(resolve(dir, '.shells/.cargo/config.toml'), 'utf8')).toContain(`EXACT_GAME_SHADERS = ${JSON.stringify(resolve(realpathSync(dir), '.shells/shaders'))}`);
  } finally { rmSync(dir, {recursive:true, force:true}); }
});
