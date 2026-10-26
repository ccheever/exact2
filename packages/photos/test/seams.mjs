// A package serves an `img` scheme (LLP 1133.019 D11 item 1, LLP 1133.001
// D8), proved on the web runtime and macOS by a fixture package outside the tree,
// `@fixture/swatches`, which serves `swatch:<colour>` pictures, drawn at the
// asked size. An app `exact new` made outside the repo depends on it by
// `file:` and shows two: a literal source and a bound one. In each engine:
//
// - under the agent, while the package's permission is `prompt`, both are
//   refused: no source, and `error` with the reason; granted, the refused
//   one is admitted and drawn;
// - granted, a bound picture is drawn at the box's device pixels;
// - revoked by the implementation, it is emptied and fires `error`
//   "revoked";
// - outside the agent the implementation is bound as in production and the
//   literal one is drawn;
// - an app that declares none of the scheme's grant lines draws none.
//
// The package's lines reuse `device.photos`: a grant line is the device
// table's (runner/src/device.rs), which a package cannot add to.
//
// usage: bun packages/photos/test/seams.mjs [chromium] [firefox] [webkit] [macos]
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, root = resolve(here, '../../..');
const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit', ...(process.platform === 'darwin' ? ['macos'] : [])];
const scratch = mkdtempSync(join(tmpdir(), 'exact-photos-seams-'));
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
const write = (dir, files) => {
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    writeFileSync(join(dir, file), typeof text === 'string' ? text : JSON.stringify(text, null, 2));
  }
};

// The package, outside the tree.
const swatches = join(scratch, 'fixture-swatches');
write(swatches, {
  'package.json': { name: '@fixture/swatches', version: '1.0.0', exports: { '.': './exact.json', './web': './web/exact.json', './apple': './apple/exact.json' } },
  'exact.json': {
    kind: 'interface', name: '@fixture/swatches', version: '1.0.0', grants: ['device.photos'], substitute: 'implementation',
    images: { scheme: 'swatch', grants: ['device.photos'], permission: 'swatches' },
    permission: [{ name: 'swatches', grants: ['device.photos'], status: 'swatchStatus', field: 'state', denied: 'NotAllowedError' }],
    shapes: { SwatchStatus: { available: 'bool', state: 'enum<granted|prompt|denied|unknown>' }, Forgotten: { name: 'string' } },
    names: [
      { source: 'swatchStatus', live: true, answers: 'SwatchStatus', substitute: { default: { available: true, state: 'prompt' } } },
      { send: 'forget', permission: false, options: { name: 'string' }, answers: 'Forgotten' },
    ],
  },
  'web/exact.json': { kind: 'implementation', implements: '@fixture/swatches@^1.0', platform: 'web', targets: ['web'], sources: { javascript: ['index.js'] } },
  'web/index.js': `// Pictures of a colour, drawn at the size asked; \`forget\` revokes one.
export function implementation(context) {
  return {
    async swatchStatus() { return { available: true, state: 'granted' }; },
    async image(source, { width, height }) {
      const canvas = new OffscreenCanvas(width, height), g = canvas.getContext('2d');
      g.fillStyle = source.slice('swatch:'.length);
      g.fillRect(0, 0, width, height);
      context.log(\`swatches: drew \${source} at \${width}x\${height}\`);
      return canvas.convertToBlob({ type: 'image/png' });
    },
    async forget({ name }) { context.revokePicture(\`swatch:\${name}\`); return { name }; },
  };
}
`,
  'apple/exact.json': { kind: 'implementation', implements: '@fixture/swatches@^1.0', platform: 'apple', targets: ['ios', 'macos'], sources: { swift: ['Swatches.swift'] }, class: 'SwatchesModule' },
  'apple/Swatches.swift': `// Pictures of a colour, drawn at the size asked; \`forget\` revokes one.
import CoreGraphics
import ExactPackage

public final class SwatchesModule: ExactPackage {
    let context: ExactPackageContext
    public init(context: ExactPackageContext) { self.context = context }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "swatchStatus": reply(["available": true, "state": "granted"])
        case "forget":
            let named = args["name"] as? String ?? ""
            context.revokePicture("swatch:\\(named)")
            reply(["name": named])
        default: reply(["error": "NotSupportedError"])
        }
    }

    public func image(_ source: String, width: Int, height: Int, fit: String, owned: Bool, reply: @escaping (CGImage?, String?) -> Void) {
        let rgb: [CGFloat] = ["red": [1, 0, 0], "blue": [0, 0, 1]][String(source.dropFirst("swatch:".count))] ?? [0, 1, 0]
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        guard let g = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return reply(nil, "UnknownError") }
        g.setFillColor(CGColor(colorSpace: space, components: rgb + [1])!)
        g.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.log("swatches: drew \\(source) at \\(width)x\\(height)")
        reply(g.makeImage(), nil)
    }
}
`,
});

const CONTRACT = `use swatchStatus, forget, SwatchStatus, Forgotten from "@fixture/swatches"

component Fixture
  state shown = ""
  state errors = ""
  state loads = 0
  state literalLoads = 0
  resource status = swatchStatus() as shape SwatchStatus else empty()
  mutation forgot as shape Forgotten
  action literalFailed(message: string)
    errors = errors + "literal:" + message + ";"
  action boundFailed(message: string)
    errors = errors + "bound:" + message + ";"
  action boundLoaded
    loads = loads + 1
  action literalLoaded
    literalLoads = literalLoads + 1
  action showBlue
    shown = "swatch:blue"
  action forgetBlue
    send forgot = forget(name="blue")
  view
    column gap=8 padding=16
      text status.state testId="state"
      text errors testId="errors"
      text loads.toString() testId="loads"
      image "swatch:red" testId="literal" width=40 height=30 error=literalFailed load=literalLoaded
      if shown != ""
        image shown testId="bound" width=50 height=20 error=boundFailed load=boundLoaded
      button press=showBlue testId="show"
        text "Show"
      button press=forgetBlue testId="forget"
        text "Forget"
`;

/** An app outside the repo with the package; `grants` its grant lines. */
function makeApp(grants) {
  const dir = mkdtempSync(join(scratch, 'app-')), app = join(dir, 'swatchfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  write(app, {
    'app.contract': CONTRACT,
    'app.ts': `export const appId = '${id}';\nexport const grants = ${JSON.stringify(grants)};\nexport const answer = (source: string) => {\n  throw new Error(\`no data source \${source}\`);\n};\n`,
    'strings/en.json': { 'purpose.swatches': 'The seam test draws swatches.' },
  });
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@fixture/swatches': `file:${swatches}` };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
  return app;
}

/** An img's source, natural size and picture attribute, by testId; `evaluate` runs an expression in the page. */
const img = (evaluate, testId) => evaluate(`(() => {
  const e = document.querySelector('[data-testid="${testId}"]');
  const i = e?.localName === 'img' ? e : e?.querySelector('img');
  return i ? { src: i.getAttribute('src'), picture: i.getAttribute('data-exact-picture'), width: i.naturalWidth, height: i.naturalHeight, dpr: devicePixelRatio } : null;
})()`);

const granted = makeApp('device.photos purpose.swatches');
process.env.EXACT_APP_DIR = granted;
process.env.EXACT2 = root;
const { open } = await import(resolve(root, 'scripts/agent.mjs'));
const browserOf = { chromium: 'chrome', firefox: 'firefox', webkit: 'webkit' };

for (const engine of engines.filter(e => Object.hasOwn(browserOf, e))) {
  const where = `${engine} agent`;
  let s;
  try {
    s = await open({ host: 'web', app: 'swatchfixture', browser: browserOf[engine] });
  } catch (e) {
    console.log(`SKIP ${where}: ${e.message.split('\n')[0]}`);
    continue;
  }
  try {
    const ev = x => s.carrier.evaluate(x);
    await s.clock('settle');
    const refused = (await s.state()).slots.errors;
    check(where, 'while prompt, the literal picture is refused with the reason', refused === 'literal:permission swatches is prompt;', JSON.stringify(refused));
    const literal = await img(ev, 'literal');
    check(where, 'and has no source: the browser fetches nothing', literal?.src == null && literal?.picture === 'swatch:red', JSON.stringify(literal));
    await s.prefer({ 'permission.swatches': 'granted' });
    await s.tap('show');
    await s.clock('settle');
    const bound = await img(ev, 'bound');
    const want = [Math.ceil(50 * (bound?.dpr ?? 1)), Math.ceil(20 * (bound?.dpr ?? 1))];
    check(where, 'granted, the bound picture is drawn at its device pixels', bound?.src?.startsWith('blob:') && bound.width === want[0] && bound.height === want[1], `${JSON.stringify(bound)} want ${want}`);
    check(where, 'and fires load', (await s.state()).slots.loads === 1, JSON.stringify((await s.state()).slots.loads));
    const again = await img(ev, 'literal');
    check(where, 'the literal picture refused while prompt is drawn once granted', again?.src?.startsWith('blob:') && (await s.state()).slots.literalLoads === 1, `${JSON.stringify(again)} ${(await s.state()).slots.literalLoads}`);
    const { extensions } = await s.state();
    check(where, 'state.extensions counts the admissions and refusals', extensions?.swatches?.pictures?.admitted >= 1 && extensions.swatches.pictures.refused >= 1, JSON.stringify(extensions?.swatches));
    await s.tap('forget');
    await s.clock('settle');
    const revoked = await img(ev, 'bound'), errors = (await s.state()).slots.errors;
    check(where, 'revoked, it is emptied and fires error "revoked"', revoked?.src == null && /bound:revoked;/.test(errors ?? ''), `${JSON.stringify(revoked)} ${errors}`);
    const logs = (await s.logs())?.lines ?? [];
    check(where, 'the journal says each refusal', logs.some(l => l.includes('image refused: swatch:red: permission swatches is prompt (LLP 1133.019 D2)')), logs.filter(l => /swatch/.test(l)).join(' | '));
  } finally {
    await s.close();
  }
}

// macOS under the agent: the package's Swift implementation, through the raster loader's admission.
if (engines.includes('macos')) {
  const where = 'macos agent';
  let s;
  try {
    run('bun', [resolve(root, 'host/apple/build.mjs'), 'swatchfixture-apple'], root, { ...process.env, EXACT_IDENTITY: process.env.EXACT_IDENTITY ?? '-' });
    s = await open({ host: 'macos', app: 'swatchfixture' });
  } catch (e) {
    console.log(`FAIL ${where}: ${e.message.split('\n')[0]}`);
    failures++;
  }
  if (s) try {
    await s.clock('settle');
    const refused = (await s.state()).slots.errors;
    // The host binds its implementations after first pixel, so the first frame's refusal can say so.
    check(where, 'while prompt, the literal picture is refused with the reason', /^literal:(permission swatches is prompt|no implementation bound);$/.test(refused ?? ''), JSON.stringify(refused));
    await s.prefer({ 'permission.swatches': 'granted' });
    await s.tap('show');
    await s.clock('settle');
    const loads = (await s.state()).slots.loads, drawn = ((await s.logs())?.lines ?? []).filter(l => /swatches: drew/.test(l));
    check(where, 'granted, the bound picture is asked at its device pixels and loads', loads === 1 && drawn.some(l => /drew swatch:blue at 100x40$/.test(l)), `${loads} ${drawn.join(' | ')}`);
    check(where, 'the literal picture refused while prompt is drawn once granted', (await s.state()).slots.literalLoads === 1 && drawn.some(l => /drew swatch:red at 80x60$/.test(l)), `${(await s.state()).slots.literalLoads} ${drawn.join(' | ')}`);
    const { extensions } = await s.state();
    check(where, 'state.extensions counts the admissions and refusals', extensions?.swatches?.pictures?.admitted >= 1 && extensions.swatches.pictures.refused >= 1, JSON.stringify(extensions?.swatches));
    await s.tap('forget');
    await s.clock('settle');
    const errors = (await s.state()).slots.errors;
    check(where, 'revoked, it fires error "revoked"', /bound:revoked;/.test(errors ?? ''), errors);
  } finally {
    await s.close();
  }
}

// Outside the agent: the implementation bound as in production, served as a deploy would.
{
  const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  const dist = join(granted, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    await page.waitForFunction(() => document.querySelector('[data-testid="literal"]')?.getAttribute('src')?.startsWith('blob:') && document.querySelector('[data-testid="literal"]').naturalWidth > 0, null, { timeout: 10000 }).catch(() => {});
    const literal = await img(x => page.evaluate(x), 'literal');
    check('chromium production', 'the bound implementation draws the literal picture', literal?.src?.startsWith('blob:') && literal.width === Math.ceil(40 * literal.dpr), JSON.stringify(literal));
  } finally {
    await browser.close();
    server.stop();
  }
}

// An app that declares none of the scheme's lines.
{
  const app = makeApp('fs.read app:/tmp');
  process.env.EXACT_APP_DIR = app;
  const s = await open({ host: 'web', app: 'swatchfixture' });
  try {
    await s.prefer({ 'permission.swatches': 'granted' });
    await s.tap('show');
    await s.clock('settle');
    const errors = (await s.state()).slots.errors;
    check('chromium agent', 'with no grant line, neither picture is drawn', /literal:grant-missing \(needs device\.photos\);/.test(errors ?? '') && /bound:grant-missing/.test(errors ?? ''), JSON.stringify(errors));
    const bound = await img(x => s.carrier.evaluate(x), 'bound');
    check('chromium agent', 'and the bound one has no source', bound?.src == null, JSON.stringify(bound));
  } finally {
    await s.close();
  }
}

rmSync(scratch, { recursive: true, force: true });
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
