// The conformance fixture the vision and translation packages share (LLP
// 1133.001 D7, LLP 1133.027 §9): an app `exact new` made outside the repo,
// its files replaced by `fixture/`'s, depending by `file:` on the three
// packages and on @exact/media-capture and @exact/camera, whose viewfinder
// a live scan rides on, and installed.
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
/** Each fixture image by name, in the package that carries it. */
export const images = {
  isbn: resolve(root, 'packages/barcode-detection/fixture/isbn-9780747532743.png'),
  upc: resolve(root, 'packages/barcode-detection/fixture/upc-a.png'),
  qr: resolve(root, 'packages/barcode-detection/fixture/qr-url.png'),
  rotated: resolve(root, 'packages/barcode-detection/fixture/rotated-6.jpg'),
  none: resolve(root, 'packages/barcode-detection/fixture/none.png'),
  quote: resolve(root, 'packages/text-detection/fixture/quote.png'),
  rotatedQuote: resolve(root, 'packages/text-detection/fixture/rotated-quote.jpg'),
};

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};

/** A fixture `EXACT_VISION_FIXTURE` names is refreshed and kept, so a native build and the Neural Engine's
 * first compile of the text recognizer (a minute or more per new app binary) are paid once while iterating. */
const kept = process.env.EXACT_VISION_FIXTURE;

/** The fixture app's directory and its bundle id. `dependencies` replace or add the app's packages by spec. */
export function makeFixture({ dependencies = {} } = {}) {
  const app = kept && existsSync(join(kept, 'app.json')) ? resolve(kept) : join(mkdtempSync(join(tmpdir(), 'exact-vision-')), 'visionfixture');
  if (app !== resolve(kept ?? '')) run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  else rmSync(join(app, 'node_modules/@exact'), { recursive: true, force: true });
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  // The camera's agent picture is an app asset (§9): the fixtures, as `assets/test/<file>`.
  mkdirSync(join(app, 'assets/test'), { recursive: true });
  for (const path of Object.values(images)) cpSync(path, join(app, 'assets/test', basename(path)));
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  const local = n => `file:${resolve(root, 'packages', n)}`;
  manifest.dependencies = {
    ...manifest.dependencies, '@exact/media-capture': local('media-capture'), '@exact/camera': local('camera'),
    '@exact/barcode-detection': local('barcode-detection'), '@exact/text-detection': local('text-detection'), '@exact/translation': local('translation'),
    ...dependencies,
  };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { app, id };
}

/** Each still: the image, the button, and what `code` lines must read (format and value), or the error. */
export const STILLS = [
  ['the ISBN by default', 'isbn', 'detect', ['ean_13 9780747532743']],
  ['the ISBN asked as upc_a finds nothing', 'isbn', 'detect-upc', []],
  ['UPC-A asked as upc_a', 'upc', 'detect-upc', ['upc_a 036000291452']],
  ['UPC-A asked as ean_13', 'upc', 'detect-ean', ['ean_13 0036000291452']],
  ['UPC-A by default', 'upc', 'detect', ['upc_a 036000291452']],
  ['the QR code', 'qr', 'detect-qr', ['qr_code https://example.com/exact/qr']],
  ['the rotated JPEG, upright', 'rotated', 'detect', ['ean_13 9780747532743']],
  ['none.png answers an empty list', 'none', 'detect', []],
];
/** The ISBN's box in the upright 468×280 image, within §9's 4% of the longer side. */
export const BOX = [44, 40, 380, 200], NEAR = 0.04 * 468;
export const boxOf = line => line.split(' ').slice(2, 6).map(Number);
export const near = b => b.length === 4 && b.every((v, i) => Math.abs(v - BOX[i]) <= NEAR);

/** Remove the fixture, unless it is the one `EXACT_VISION_FIXTURE` keeps. */
export const discard = app => { if (resolve(app) !== resolve(kept ?? '')) rmSync(dirname(app), { recursive: true, force: true }); };

/** The text a `tree` shows for each node of `testId`, in order. */
export const shown = (out, testId) => [...out.matchAll(new RegExp(`\\[${testId}\\] "((?:[^"\\\\]|\\\\.)*)"`, 'g'))].map(m => m[1]);

/** A suite's PASS or FAIL line; `failed()` counts the FAILs. */
export function checker() {
  let failures = 0;
  const check = (where, name, ok, detail) => {
    if (!ok) failures++;
    console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
  };
  return { check, failed: () => failures };
}
