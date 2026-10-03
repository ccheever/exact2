#!/usr/bin/env bun
// Writes every model, texture and sky under ../art from code; the game bake turns
// them into .model and .tex assets. Deterministic: rerun after editing a source.
//   bun game/games/rivals/art-src/gen.mjs
import { mkdirSync, readFileSync, writeFileSync, statSync, readdirSync, rmSync } from 'node:fs';
import { Gltf } from './gltf.mjs';
import { resolve } from 'node:path';
import * as tex from './textures.mjs';
import * as models from './models.mjs';

const here = import.meta.dir, art = resolve(here, '../art');
mkdirSync(art, { recursive: true });
const write = (name, bytes) => writeFileSync(resolve(art, name), bytes);
const layout = JSON.parse(readFileSync(resolve(here, '../arena.json'), 'utf8'));

// Material textures are embedded in the models that use them: a PNG under art/
// would also bake as a standalone sprite texture. Only the skies stand alone.
const images = Gltf.images;
for (const [name, set] of [['panel', tex.panel()], ['floor', tex.floor()], ['crate', tex.crate()], ['deck', tex.deck()], ['gunmetal', tex.gunmetal()]]) {
  images[`${name}.png`] = set.albedo.png(true);
  images[`${name}_n.png`] = set.normal.png(false);
  images[`${name}_mr.png`] = set.mr.png(false);
  if (set.emissive) images[`${name}_e.png`] = set.emissive.png(true);
}
images['logo_red.png'] = tex.logo([0.75, 0.06, 0.05], [0.25, 0.01, 0.02]).png(true);
images['logo_blue.png'] = tex.logo([0.05, 0.3, 0.85], [0.01, 0.06, 0.25]).png(true);
for (const f of readdirSync(art)) rmSync(resolve(art, f));
write('sky.png', tex.sky(false).png(true));
write('sky_night.png', tex.sky(true).png(true));

const out = { arena: models.arena(layout), rifle: models.rifle(), mag: models.rifleMag(), launcher: models.launcher(),
  knife: models.knife(), rocket: models.rocket(), flash: models.flash() };
// One soldier per bot colour, the order of the game's COLORS.
const teams = [[0.86, 0.22, 0.24], [0.94, 0.62, 0.12], [0.55, 0.3, 0.85], [0.15, 0.7, 0.45],
  [0.9, 0.35, 0.65], [0.2, 0.55, 0.9], [0.65, 0.65, 0.2], [0.4, 0.8, 0.85]];
teams.forEach((c, i) => { out[`soldier${i}`] = models.soldier(c, c.map((x) => x * 0.25 + 0.06)); });
for (const [name, gltf] of Object.entries(out)) gltf.write(resolve(art, `${name}.gltf`));
let total = 0;
for (const f of readdirSync(art)) total += statSync(resolve(art, f)).size;
console.log(`art/: ${readdirSync(art).length} files, ${(total / 1024).toFixed(0)} KiB`);
