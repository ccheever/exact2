//! `storage.fs.compressImage` in real Chrome (LLP 1069.002 A1.3, A1.7): the
//! web host's `storage-fs.js` over `storage-image.js`, in the page and in a
//! module worker, and a Rust source's request through `storage-request.js`;
//! and `transformImage` and `compressImage`'s edit there (LLP 1133.019 §11's
//! image conformance), with Firefox and WebKit where installed.
use std::path::Path;
use std::process::Command;

const GRANTS: [&str; 2] = [
    "fs.read app:/\nfs.write app:/data\nfs.write app:/tmp",
    "fs.read app:/tmp",
];

#[test]
fn chrome_compresses_an_image_upright_within_budget_and_refuses_by_name() {
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
    let result = Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
        .args(["--input-type=module", "-e", PROBE])
        .env(
            "EXACT_GRANT_SETS",
            serde_json::Value::Object(sets).to_string(),
        )
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()
        .unwrap();
    if let Some(why) = String::from_utf8_lossy(&result.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("missing chrome: "))
        .map(str::to_owned)
    {
        missing("chrome", &why);
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        result.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reports: serde_json::Value =
        serde_json::from_str(stdout.trim().lines().last().unwrap()).unwrap();
    for engine in ["chrome", "firefox", "webkit"] {
        let report = &reports[engine];
        if let Some(why) = report["unavailable"]
            .as_str()
            .filter(|_| engine != "chrome")
        {
            missing(
                engine,
                &format!("compressImage on {engine} unavailable: {why}"),
            );
            continue;
        }
        check(engine, report);
    }
}

/// One engine's report: the page and the worker, then the Rust request.
fn check(engine: &str, report: &serde_json::Value) {
    for realm in ["page", "worker"] {
        let r = &report[realm];
        assert_eq!(
            r["record"]["path"], "app:/data/out.jpg",
            "{engine} {realm}: {r}"
        );
        assert_eq!(r["record"]["type"], "image/jpeg", "{engine} {realm}");
        assert_eq!(
            (&r["record"]["width"], &r["record"]["height"]),
            (&48.into(), &64.into()),
            "{engine} {realm}"
        );
        assert_eq!(r["record"]["size"], r["readBack"], "{engine} {realm}");
        assert_eq!(
            (&r["decoded"][0], &r["decoded"][1]),
            (&48.into(), &64.into()),
            "{engine} {realm}"
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
            "{engine} {realm}: top-left {px:?}"
        );
        assert_eq!(
            r["sourceExif"], true,
            "{engine} {realm}: the tag reader is blind"
        );
        assert_eq!(
            r["exif"], false,
            "{engine} {realm}: the JPEG carries the source's GPS, date or orientation"
        );
        assert_eq!(r["scaled"], serde_json::json!([24, 32]), "{engine} {realm}");
        let noise = &r["noise"];
        assert!(
            noise["size"].as_u64().unwrap() <= 120_000,
            "{engine} {realm}: {noise}"
        );
        assert!(
            noise["width"].as_u64().unwrap() < 1000,
            "{engine} {realm}: noise did not shrink: {noise}"
        );
        assert_eq!(
            r["picked"],
            serde_json::json!([48, 64]),
            "{engine} {realm}: a picked File entry"
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
            "{engine} {realm}: {r}"
        );
        assert_eq!(
            r["untouched"], 3,
            "{engine} {realm}: a refusal changed the destination"
        );
        assert_eq!(
            r["huge"],
            serde_json::json!(["too-large", 0]),
            "{engine} {realm}: decoded before the header check"
        );
        assert!(
            r["clear"]
                .as_array()
                .unwrap()
                .iter()
                .all(|v| v.as_u64().unwrap() > 245),
            "{engine} {realm}: {}",
            r["clear"]
        );
        assert_eq!(
            r["png"],
            serde_json::json!({"type": "image/png", "size": [8, 8], "left": 0, "right": 255}),
            "{engine} {realm}: a PNG lost its alpha"
        );
        assert_eq!(r["badType"], "TypeError", "{engine} {realm}");
        transformed(engine, realm, &r["transform"]);
    }
    // A Rust source's transform: D14's options beside the paths, its record back.
    assert_eq!(
        report["rustTransform"],
        serde_json::json!({"path":"app:/data/rust-turned.jpg","type":"image/jpeg","width":64,"height":48,
            "colorSpace":"srgb","hdr":false,"metadata":"none","sized":true}),
        "{engine}: {report}"
    );
    assert_eq!(report["rustPng"], "image/png", "{engine}: {report}");
    // A null `type` is absent, as the native request reads it.
    assert_eq!(report["rustNull"], "image/jpeg", "{engine}: {report}");
    assert_eq!(
        report["rust"],
        serde_json::json!({"path":"app:/data/rust.jpg","type":"image/jpeg","width":24,"height":32,"sized":true}),
        "{engine}: {report}"
    );
    // The refusal names the operation first, then how to grant it.
    assert!(
        report["rustDenied"]
            .as_str()
            .is_some_and(|m| m.starts_with("denied: fs.write")),
        "{engine}: {report}"
    );
    assert_eq!(
        report["rustDoc"], "compressImage: needs app:/ paths",
        "{engine}: {report}"
    );
}

/// LLP 1133.019 §11's image conformance on the canvas codec.
fn transformed(engine: &str, realm: &str, t: &serde_json::Value) {
    let rgb = |v: &serde_json::Value| -> Vec<u64> {
        v.as_array().unwrap()[..3]
            .iter()
            .map(|c| c.as_u64().unwrap())
            .collect()
    };
    let blue = |v: &serde_json::Value| {
        let c = rgb(v);
        c[2] > 200 && c[0] < 60 && c[1] < 60
    };
    let red = |v: &serde_json::Value| {
        let c = rgb(v);
        c[0] > 200 && c[1] < 60 && c[2] < 60
    };
    // A `to` without an extension gains the type's; a canvas keeps nothing,
    // and says so whatever was asked.
    assert_eq!(
        t["turned"]["record"],
        serde_json::json!({"path":"app:/data/turned.jpg","type":"image/jpeg","size":t["turned"]["record"]["size"],
            "width":64,"height":48,"colorSpace":"srgb","hdr":false,"metadata":"none"}),
        "{engine} {realm}: {t}"
    );
    assert!(
        blue(&t["turned"]["top"]) && red(&t["turned"]["bottom"]),
        "{engine} {realm}: rotate 90 puts blue on top: {t}"
    );
    assert_eq!(t["turned"]["exif"], false, "{engine} {realm}: {t}");
    assert!(
        red(&t["flipped"]["left"]) && blue(&t["flipped"]["right"]),
        "{engine} {realm}: flip puts blue on the right: {t}"
    );
    assert_eq!(
        t["cropped"]["size"],
        serde_json::json!([24, 64]),
        "{engine} {realm}"
    );
    assert!(
        t["cropped"]["corners"].as_array().unwrap().iter().all(blue),
        "{engine} {realm}: the crop is all blue: {t}"
    );
    assert_eq!(
        t["free"]["size"],
        serde_json::json!([74, 80]),
        "{engine} {realm}: {t}"
    );
    assert_eq!(
        t["free"]["png"], 0,
        "{engine} {realm}: a free turn's PNG corner is clear: {t}"
    );
    assert!(
        rgb(&t["free"]["jpeg"]).iter().all(|&c| c > 245),
        "{engine} {realm}: a free turn's JPEG corner is white: {t}"
    );
    assert_eq!(
        t["compressed"]["size"],
        serde_json::json!([64, 24]),
        "{engine} {realm}: {t}"
    );
    assert!(blue(&t["compressed"]["topLeft"]), "{engine} {realm}: {t}");
    assert_eq!(
        t["codes"],
        serde_json::json!(["outside", "unsupported", "TypeError", "denied", "outside"]),
        "{engine} {realm}: {t}"
    );
}

const PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { chromium } from './scripts/agent-launch.mjs';
const { executable: browserExecutable, unavailable: browserMissing } = chromium();
if (browserMissing) { console.log(`missing chrome: ${browserMissing}`); process.exit(0); }
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
const child = spawn(browserExecutable, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
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
  // The source's metadata is gone: no GPS, capture date or turned
  // orientation. An encoder may write EXIF of its own (WebKit's does: its
  // colour space and size), so the tags are read, not the block's presence.
  const tags = jpeg => {
    const v = new DataView(jpeg), found = {};
    for (let pos = 2; pos + 4 < v.byteLength && v.getUint8(pos) === 0xff;) {
      const marker = v.getUint8(pos + 1), length = v.getUint16(pos + 2);
      if (marker === 0xe1 && v.getUint32(pos + 4) === 0x45786966) {
        const base = pos + 10, le = v.getUint16(base) === 0x4949;
        const u16 = i => v.getUint16(base + i, le), u32 = i => v.getUint32(base + i, le);
        const ifd = at => { for (let k = 0, n = u16(at); k < n; k++) { const e = at + 2 + 12 * k, tag = u16(e); found[tag] = tag === 0x0112 ? u16(e + 8) : u32(e + 8); } };
        ifd(u32(4));
        if (found[0x8769]) ifd(found[0x8769]);
      }
      if (marker === 0xda) break;
      pos += 2 + length;
    }
    return found;
  };
  const meta = tags(back);
  report.exif = Boolean(meta[0x8825] || meta[0x9003] || (meta[0x0112] && meta[0x0112] !== 1));
  const source = tags(fixture); // the reader finds them in the source
  report.sourceExif = Boolean(source[0x8825] && source[0x9003] && source[0x0112] === 6);
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
  // `type: "image/png"` (A1.11): clear on the left half, red on the right, kept.
  const half = new OffscreenCanvas(16, 16), halfContext = half.getContext('2d');
  halfContext.fillStyle = '#f00'; halfContext.fillRect(8, 0, 8, 16);
  await fs.writeFile('app:/tmp/half.png', await (await half.convertToBlob({ type: 'image/png' })).arrayBuffer());
  const png = await fs.compressImage('app:/tmp/half.png', 'app:/tmp/half-out.png', { maxDimension: 8, maxBytes: 100000, type: 'image/png' });
  const alpha = new OffscreenCanvas(8, 8).getContext('2d');
  alpha.drawImage(await createImageBitmap(new Blob([await fs.readFile('app:/tmp/half-out.png')], { type: 'image/png' })), 0, 0);
  report.png = { type: png.type, size: [png.width, png.height], left: alpha.getImageData(0, 0, 1, 1).data[3], right: alpha.getImageData(7, 0, 1, 1).data[3] };
  report.badType = await code(fs.compressImage('app:/tmp/half.png', 'app:/tmp/x.webp', { maxDimension: 8, maxBytes: 100, type: 'image/webp' }));
  // `transformImage` and `compressImage`'s edit (LLP 1133.019 §11): the
  // fixture upright is 48×64, blue on the left.
  const at = async (path, type, points) => {
    const b = await createImageBitmap(new Blob([await fs.readFile(path)], { type }));
    const c = new OffscreenCanvas(b.width, b.height).getContext('2d');
    c.drawImage(b, 0, 0);
    return { size: [b.width, b.height], points: points.map(([x, y]) => [...c.getImageData(x, y, 1, 1).data]) };
  };
  const t = report.transform = {};
  const turned = await fs.transformImage('app:/tmp/in.jpg', 'app:/data/turned', { rotate: 90, metadata: 'keep' });
  const [top, bottom] = (await at(turned.path, 'image/jpeg', [[32, 2], [32, 45]])).points;
  const turnedTags = tags(await fs.readFile(turned.path));
  t.turned = { record: turned, top, bottom, exif: Boolean(turnedTags[0x8825] || turnedTags[0x9003]) };
  const flipped = await fs.transformImage('app:/tmp/in.jpg', 'app:/data/flipped.jpg', { flip: 'horizontal' });
  const [left, right] = (await at(flipped.path, 'image/jpeg', [[2, 32], [45, 32]])).points;
  t.flipped = { left, right };
  const cropped = await fs.transformImage('app:/tmp/in.jpg', 'app:/data/cropped.jpg', { crop: { x: 0, y: 0, width: 24, height: 64 } });
  t.cropped = { size: [cropped.width, cropped.height], corners: (await at(cropped.path, 'image/jpeg', [[1, 1], [22, 1], [1, 62], [22, 62]])).points };
  // 48 cos 30 + 64 sin 30 = 73.6, 48 sin 30 + 64 cos 30 = 79.4.
  const free = await fs.transformImage('app:/tmp/in.jpg', 'app:/data/free', { rotate: 30, type: 'image/png' });
  const freeJpeg = await fs.transformImage('app:/tmp/in.jpg', 'app:/data/free.jpg', { rotate: 30 });
  t.free = { size: [free.width, free.height], png: (await at(free.path, 'image/png', [[0, 0]])).points[0][3], jpeg: (await at(freeJpeg.path, 'image/jpeg', [[0, 0]])).points[0] };
  const compressed = await fs.compressImage('app:/tmp/in.jpg', 'app:/data/compressed.jpg', { maxDimension: 100, maxBytes: 2_000_000, rotate: 90, crop: { x: 0, y: 0, width: 64, height: 24 } });
  t.compressed = { size: [compressed.width, compressed.height], topLeft: (await at(compressed.path, 'image/jpeg', [[2, 2]])).points[0] };
  t.codes = [
    await code(fs.transformImage('app:/tmp/in.jpg', 'app:/data/x.jpg', { crop: { x: 100, y: 0, width: 4, height: 4 } })),
    await code(fs.transformImage('app:/tmp/in.jpg', 'app:/data/x.img', { type: 'image/x-unknown' })),
    await code(fs.transformImage('app:/tmp/in.jpg', 'app:/data/x.jpg', { flip: 'both' })),
    await code(fs.transformImage('app:/tmp/in.jpg', 'app:/cache/x.jpg')),
    await code(fs.compressImage('app:/tmp/in.jpg', 'app:/data/x.jpg', { maxDimension: 10, maxBytes: 10000, crop: { x: 0, y: 100, width: 4, height: 4 } })),
  ];
  fs.dispose();
  return report;
};
// What each engine is asked: the checks in the page, then in a module
// worker, then a Rust source's request through storage-request.js.
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
    const rustPng = decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.compressImage', args: { path: 'app:/tmp/in.jpg', destination: 'app:/data/rust.png', maxDimension: 32, maxBytes: 2000000, type: 'image/png' } })));
    const rustNull = decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.compressImage', args: { path: 'app:/tmp/in.jpg', destination: 'app:/data/rust-null.jpg', maxDimension: 32, maxBytes: 2000000, type: null } })));
    const rustDoc = decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.compressImage', args: { path: 'doc:/1/in.jpg', destination: 'app:/data/x.jpg', maxDimension: 32, maxBytes: 2000000 } })));
    const turned = decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.transformImage', args: { path: 'app:/tmp/in.jpg', destination: 'app:/data/rust-turned', rotate: 90 } })));
    const { size, ...rustTransform } = turned;
    requests.dispose();
    return JSON.stringify({ page, worker: reply.ok, rust: rust.error ? rust : { path: rust.path, type: rust.type, width: rust.width, height: rust.height, sized: rust.size > 0 }, rustDenied: rustDenied.error, rustDoc: rustDoc.error, rustPng: rustPng.error ?? rustPng.type, rustNull: rustNull.error ?? rustNull.type,
      rustTransform: turned.error ? turned : { ...rustTransform, sized: size > 0 } });
  })()`;
const reports = {};
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const page = targetInfos.find(t => t.type === 'page') ?? await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: page.targetId, flatten: true });
  const call = (method, params) => cdp.send(method, params, sessionId);
  await call('Page.enable');
  await call('Page.navigate', { url: origin + '/' });
  await new Promise(r => setTimeout(r, 300));
  const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  reports.chrome = JSON.parse(result.result.value);
} finally {
  try { process.kill(-child.pid, 'SIGKILL'); } catch {}
  await exited;
  rmSync(profile, { recursive: true, force: true });
}
// Firefox and WebKit, through Playwright as conform.mjs and the agent's
// carriers launch them, when they are installed (bunx playwright@1.64.1
// install firefox webkit); else said, not failed.
try {
  const playwright = await import('playwright-core');
  for (const name of ['firefox', 'webkit']) {
    // A persistent profile: an ephemeral Firefox context cannot keep a Blob
    // in IndexedDB, which a picked entry is (LLP 1069.002 D5).
    const dir = mkdtempSync(resolve(tmpdir(), `exact-compress-${name}-`));
    let context;
    try { context = await playwright[name].launchPersistentContext(dir); }
    catch (error) { reports[name] = { unavailable: String(error.message).split('\n')[0] }; rmSync(dir, { recursive: true, force: true }); continue; }
    try {
      const page = context.pages()[0] ?? await context.newPage();
      await page.goto(origin + '/');
      reports[name] = JSON.parse(await page.evaluate(expression));
    } finally { await context.close(); rmSync(dir, { recursive: true, force: true }); }
  }
} finally {
  server.closeAllConnections(); server.close();
}
console.log(JSON.stringify(reports));
"#;

/// A prerequisite this run lacks: skipped, and a failure under EXACT_REQUIRED=1,
/// which the async lane sets (scripts/coverage.mjs).
fn missing(kind: &str, why: &str) {
    assert_ne!(
        std::env::var("EXACT_REQUIRED").as_deref(),
        Ok("1"),
        "missing {kind}: {why}"
    );
    eprintln!("SKIP missing {kind}: {why}");
}
