#!/usr/bin/env bun
// Writes the art pass's models, textures and skies under ../art from code; the
// game bake turns them into .model and .tex assets. Deterministic: rerun after
// editing a source. Everything under art/ is this generator's output.
//   bun game/games/rivals/art-src/gen.mjs
import { mkdirSync, readFileSync, writeFileSync, statSync, readdirSync, rmSync } from 'node:fs';
import { Gltf } from './gltf.mjs';
import { resolve, dirname } from 'node:path';
import * as tex from './textures.mjs';
import * as models from './models.mjs';

const here = import.meta.dir, art = resolve(here, '../art');
rmSync(art, { recursive: true, force: true });
const write = (name, bytes) => {
  mkdirSync(dirname(resolve(art, name)), { recursive: true });
  writeFileSync(resolve(art, name), bytes);
};
const layout = JSON.parse(readFileSync(resolve(here, '../arena.json'), 'utf8'));

// Shared textures: art/textures/ is colour (sRGB), art/data/ linear (normal and
// metallic-roughness maps). A model names them by URI, so each bakes once, by
// content, however many models sample it; art/data/rgbm/ holds the skies.
const image = (path, canvas, colour) => { write(path, canvas.png(colour)); Gltf.images.add(path); };
for (const [name, set] of [['panel', tex.panel()], ['floor', tex.floor()], ['crate', tex.crate()], ['deck', tex.deck()], ['gunmetal', tex.gunmetal()]]) {
  image(`textures/${name}.png`, set.albedo, true);
  image(`data/${name}_n.png`, set.normal, false);
  image(`data/${name}_mr.png`, set.mr, false);
  if (set.emissive) image(`textures/${name}_e.png`, set.emissive, true);
}
image('textures/logo_red.png', tex.logo([0.75, 0.06, 0.05], [0.25, 0.01, 0.02]), true);
image('textures/logo_blue.png', tex.logo([0.05, 0.3, 0.85], [0.01, 0.06, 0.25]), true);
image('textures/smoke.png', tex.smoke(), true);
write('data/rgbm/sky.png', tex.sky(false).png(false));
write('data/rgbm/sky_night.png', tex.sky(true, 4).png(false));

const out = { arena: models.arena(layout), rifle: models.rifle(), mag: models.rifleMag(), launcher: models.launcher(),
  knife: models.knife(), rocket: models.rocket(), flash: models.flash() };
for (const [part, gltf] of Object.entries(models.soldier())) out[`soldier_${part}`] = gltf;
for (const [name, gltf] of Object.entries(out)) gltf.write(resolve(art, `${name}.gltf`));
let total = 0, files = 0;
const walk = (dir) => {
  for (const f of readdirSync(dir, { withFileTypes: true })) {
    if (f.isDirectory()) walk(resolve(dir, f.name));
    else { total += statSync(resolve(dir, f.name)).size; files++; }
  }
};
walk(art);
console.log(`art/: ${files} files, ${(total / 1024).toFixed(0)} KiB`);
