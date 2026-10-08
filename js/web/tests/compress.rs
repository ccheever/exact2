//! `storage.fs.compressImage` in real Chrome (LLP 1069.002 A1.3, A1.7): the
//! web host's `storage-fs.js` over `storage-image.js`, in the page and in a
//! module worker, and a Rust source's request through `storage-request.js`.
use std::path::Path;
use std::process::Command;

const GRANTS: [&str; 2] = [
    "fs.read app:/\nfs.write app:/data\nfs.write app:/tmp",
    "fs.read app:/tmp",
];

#[test]
fn chrome_compresses_an_image_upright_within_budget_and_refuses_by_name() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !Path::new(&chrome).exists() {
        eprintln!("compressImage browser check unavailable: set CHROME");
        return;
    }
    let sets = GRANTS
        .iter()
        .map(|spec| {
            (
                spec.to_string(),
                serde_json::from_str::<serde_json::Value>(&exact_runner::grants::normalized_json(
                    spec,
                ))
                .unwrap(),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let result = Command::new("bun")
        .args(["--input-type=module", "-e", PROBE])
        .env("CHROME", chrome)
        .env(
            "EXACT_GRANT_SETS",
            serde_json::Value::Object(sets).to_string(),
        )
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        result.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_str(stdout.trim().lines().last().unwrap()).unwrap();
    for realm in ["page", "worker"] {
        let r = &report[realm];
        assert_eq!(r["record"]["path"], "app:/data/out.jpg", "{realm}: {r}");
        assert_eq!(r["record"]["type"], "image/jpeg", "{realm}");
        assert_eq!(
            (&r["record"]["width"], &r["record"]["height"]),
            (&48.into(), &64.into()),
            "{realm}"
        );
        assert_eq!(r["record"]["size"], r["readBack"], "{realm}");
        assert_eq!(
            (&r["decoded"][0], &r["decoded"][1]),
            (&48.into(), &64.into()),
            "{realm}"
        );
        // Stored red over blue, turned a quarter clockwise: blue on the left.
        let px: Vec<u64> = r["topLeft"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert!(
            px[2] > 200 && px[0] < 60 && px[1] < 60,
            "{realm}: top-left {px:?}"
        );
        assert_eq!(r["exif"], false, "{realm}: the JPEG carries no EXIF");
        assert_eq!(r["scaled"], serde_json::json!([24, 32]), "{realm}");
        let noise = &r["noise"];
        assert!(
            noise["size"].as_u64().unwrap() <= 120_000,
            "{realm}: {noise}"
        );
        assert!(
            noise["width"].as_u64().unwrap() < 1000,
            "{realm}: noise did not shrink: {noise}"
        );
        assert_eq!(
            r["picked"],
            serde_json::json!([48, 64]),
            "{realm}: a picked File entry"
        );
        assert_eq!(
            r["codes"],
            serde_json::json!([
                "denied",
                "undecodable",
                "unfit",
                "failed",
                "TypeError",
                "TypeError",
                "ENOENT"
            ]),
            "{realm}: {r}"
        );
        assert_eq!(
            r["untouched"], 3,
            "{realm}: a refusal changed the destination"
        );
        assert_eq!(
            r["huge"],
            serde_json::json!(["too-large", 0]),
            "{realm}: decoded before the header check"
        );
        assert!(
            r["clear"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v.as_u64().unwrap() > 245),
            "{realm}: {}",
            r["clear"]
        );
    }
    assert_eq!(
        report["rust"],
        serde_json::json!({"path":"app:/data/rust.jpg","type":"image/jpeg","width":24,"height":32,"sized":true}),
        "{report}"
    );
    assert_eq!(report["rustDenied"], "denied: fs.write", "{report}");
    assert_eq!(
        report["rustDoc"], "compressImage: needs app:/ paths",
        "{report}"
    );
}

const PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { webHostFiles } from './scripts/app.mjs';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
const routes = Object.fromEntries(Object.entries(webHostFiles()).map(([name, source]) => ['/' + name, source]));
const server = createServer((req, res) => {
  const path = new URL(req.url, 'http://fixture.invalid').pathname;
  if (path === '/') { res.setHeader('content-type', 'text/html'); res.end('<!doctype html><title>compress</title>'); return; }
  if (path === '/fixture.jpg') { res.setHeader('content-type', 'image/jpeg'); res.end(readFileSync('scripts/fixtures/picker/oriented-gps.jpg')); return; }
  if (!routes[path]) { res.writeHead(404); res.end(); return; }
  res.setHeader('content-type', path.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
  res.end(readFileSync(routes[path]));
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;
const profile = mkdtempSync(resolve(tmpdir(), 'exact-compress-browser-'));
const child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
const cdp = new Cdp(child.stdio[3], child.stdio[4]);
const exited = new Promise(r => child.on('exit', () => { cdp.fail('browser closed'); r(); }));
// The same checks in the page and in a module worker: `realm` runs them
// with `origin`'s modules and the grant sets the Rust side normalized.
const checks = async (origin, sets, app) => {
  const { createFileSystem, createFileStore } = await import(origin + '/storage-fs.js');
  const { createGrantSet } = await import(origin + '/grant-admission.js');
  const keys = Object.keys(sets);
  const fs = createFileSystem(app, createGrantSet(sets[keys[0]]));
  const fixture = await (await fetch(origin + '/fixture.jpg')).arrayBuffer();
  await fs.writeFile('app:/tmp/in.jpg', fixture);
  const report = {};
  report.record = await fs.compressImage('app:/tmp/in.jpg', 'app:/data/out.jpg', { maxDimension: 4000, maxBytes: 2_000_000 });
  const back = await fs.readFile('app:/data/out.jpg');
  report.readBack = back.byteLength;
  const bitmap = await createImageBitmap(new Blob([back], { type: 'image/jpeg' }));
  report.decoded = [bitmap.width, bitmap.height];
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height), context = canvas.getContext('2d');
  context.drawImage(bitmap, 0, 0);
  report.topLeft = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3);
  const bytes = new Uint8Array(back), exif = [0x45, 0x78, 0x69, 0x66, 0, 0];
  report.exif = bytes.some((_, i) => exif.every((b, j) => bytes[i + j] === b));
  const scaled = await fs.compressImage('app:/tmp/in.jpg', 'app:/tmp/small.jpg', { maxDimension: 32, maxBytes: 2_000_000 });
  report.scaled = [scaled.width, scaled.height];
  // Noise, which JPEG cannot compress: deterministic, so every run shrinks alike.
  const noise = new OffscreenCanvas(1000, 800), n = noise.getContext('2d'), image = n.createImageData(1000, 800);
  let seed = 0x12345678;
  for (let i = 0; i < image.data.length; i++) { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; image.data[i] = i % 4 === 3 ? 255 : seed >>> 24; }
  n.putImageData(image, 0, 0);
  await fs.writeFile('app:/tmp/noise.png', await (await noise.convertToBlob({ type: 'image/png' })).arrayBuffer());
  const shrunk = await fs.compressImage('app:/tmp/noise.png', 'app:/tmp/noise.jpg', { maxDimension: 1000, maxBytes: 120_000 });
  report.noise = { width: shrunk.width, height: shrunk.height, size: shrunk.size };
  // A picked entry is the browser's File (LLP 1069.002 D5).
  await createFileStore(app).putBlob('app:/tmp/picked.jpg', new File([fixture], 'IMG_1.jpg', { type: 'image/jpeg' }));
  const picked = await fs.compressImage('app:/tmp/picked.jpg', 'app:/tmp/picked-out.jpg', { maxDimension: 4000, maxBytes: 2_000_000 });
  report.picked = [picked.width, picked.height];
  await fs.writeFile('app:/tmp/note.txt', new TextEncoder().encode('not an image'));
  await fs.writeFile('app:/data/keep.jpg', new Uint8Array([1, 2, 3]));
  const code = async promise => { try { await promise; return 'ok'; } catch (e) { return e instanceof TypeError ? 'TypeError' : e.code ?? (/^denied: /.test(e.message) ? 'denied' : 'failed'); } }; // the adapters name a denial
  report.codes = [
    await code(fs.compressImage('app:/tmp/in.jpg', 'app:/cache/out.jpg', { maxDimension: 10, maxBytes: 10 })),
    await code(fs.compressImage('app:/tmp/note.txt', 'app:/data/keep.jpg', { maxDimension: 10, maxBytes: 10000 })),
    await code(fs.compressImage('app:/tmp/in.jpg', 'app:/data/keep.jpg', { maxDimension: 64, maxBytes: 1 })),
    await code(fs.compressImage('app:/tmp/in.jpg', 'doc:/1/out.jpg', { maxDimension: 10, maxBytes: 10 })),
    await code(fs.compressImage('app:/tmp/in.jpg', 'app:/data/keep.jpg', { maxDimension: 0, maxBytes: 10 })),
    await code(fs.compressImage('app:/tmp/in.jpg', 'app:/data/keep.jpg')),
    await code(fs.compressImage('app:/tmp/absent.jpg', 'app:/data/keep.jpg', { maxDimension: 10, maxBytes: 10 })),
  ];
  // A header over the pixel limit is refused before the browser decodes.
  const huge = new Uint8Array(fixture.slice(0)), sof = huge.findIndex((b, i) => b === 0xff && huge[i + 1] === 0xc0);
  new DataView(huge.buffer).setUint16(sof + 5, 9000); new DataView(huge.buffer).setUint16(sof + 7, 9000);
  await fs.writeFile('app:/tmp/huge.jpg', huge);
  const decoder = globalThis.createImageBitmap; let decodes = 0;
  globalThis.createImageBitmap = (...args) => { decodes++; return decoder(...args); };
  report.huge = [await code(fs.compressImage('app:/tmp/huge.jpg', 'app:/data/keep.jpg', { maxDimension: 10, maxBytes: 10000 })), decodes];
  globalThis.createImageBitmap = decoder;
  report.untouched = (await fs.stat('app:/data/keep.jpg')).size;
  // Transparency becomes white.
  const clear = new OffscreenCanvas(8, 8); clear.getContext('2d'); // transparent black
  await fs.writeFile('app:/tmp/clear.png', await (await clear.convertToBlob({ type: 'image/png' })).arrayBuffer());
  await fs.compressImage('app:/tmp/clear.png', 'app:/tmp/clear.jpg', { maxDimension: 8, maxBytes: 100000 });
  const white = new OffscreenCanvas(8, 8).getContext('2d');
  white.drawImage(await createImageBitmap(new Blob([await fs.readFile('app:/tmp/clear.jpg')], { type: 'image/jpeg' })), 0, 0);
  report.clear = [...white.getImageData(0, 0, 1, 1).data].slice(0, 3);
  fs.dispose();
  return report;
};
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const page = targetInfos.find(t => t.type === 'page') ?? await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: page.targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  await call('Page.enable');
  await call('Page.navigate', { url: origin + '/' });
  await new Promise(r => setTimeout(r, 300));
  const expression = `(async () => {
    const sets = ${process.env.EXACT_GRANT_SETS}, checks = ${checks.toString()};
    const page = await checks(${JSON.stringify(origin)}, sets, 'test.compress.page');
    const source = 'const checks = ' + checks.toString() + ';onmessage = async e => { try { postMessage({ ok: await checks(...e.data) }); } catch (error) { postMessage({ error: String(error && error.stack || error) }); } };';
    const worker = new Worker(URL.createObjectURL(new Blob([source], { type: 'text/javascript' })), { type: 'module' });
    const reply = await new Promise(resolve => { worker.onmessage = e => resolve(e.data); worker.postMessage([${JSON.stringify(origin)}, sets, 'test.compress.worker']); });
    worker.terminate();
    if (reply.error) throw new Error('worker: ' + reply.error);
    // A Rust source's request (storage-request.js), under each grant set.
    globalThis.exact ??= {};
    const { createStorageRequests } = await import(${JSON.stringify(origin)} + '/storage-request.js');
    const { createGrantSet } = await import(${JSON.stringify(origin)} + '/grant-admission.js');
    const keys = Object.keys(sets), decode = bytes => JSON.parse(new TextDecoder().decode(bytes));
    const { storageKey } = await import(${JSON.stringify(origin)} + '/storage-environment.js');
    const { createFileSystem } = await import(${JSON.stringify(origin)} + '/storage-fs.js');
    const files = createFileSystem(storageKey('test.compress.rust'), createGrantSet(sets[keys[0]]));
    await files.writeFile('app:/tmp/in.jpg', await (await fetch('/fixture.jpg')).arrayBuffer());
    files.dispose();
    const requests = createStorageRequests('test.compress.rust', createGrantSet(sets[keys[0]]));
    const ask = (destination, scope) => requests.run(JSON.stringify({ version: 1, op: 'fs.compressImage', args: { path: 'app:/tmp/in.jpg', destination, maxDimension: 32, maxBytes: 2000000 } }), scope);
    const rust = decode(await ask('app:/data/rust.jpg'));
    const rustDenied = decode(await ask('app:/data/denied.jpg', createGrantSet(sets[keys[1]])));
    const rustDoc = decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.compressImage', args: { path: 'doc:/1/in.jpg', destination: 'app:/data/x.jpg', maxDimension: 32, maxBytes: 2000000 } })));
    requests.dispose();
    return JSON.stringify({ page, worker: reply.ok, rust: rust.error ? rust : { path: rust.path, type: rust.type, width: rust.width, height: rust.height, sized: rust.size > 0 }, rustDenied: rustDenied.error, rustDoc: rustDoc.error });
  })()`;
  const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  console.log(result.result.value);
} finally {
  try { process.kill(-child.pid, 'SIGKILL'); } catch {}
  await exited;
  server.closeAllConnections(); server.close();
  rmSync(profile, { recursive: true, force: true });
}
"#;
