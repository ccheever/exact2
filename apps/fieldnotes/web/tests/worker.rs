//! Real Chrome coverage of the shipped Fieldnotes web app on its placement
//! (LLP 1027.002 §5 steps 4 and 5): the notebook drive, then the frame gaps
//! the page's main thread shows while a backup of 1,000 notes runs beside
//! wheel scrolling, judged against the display's own frame interval, at 1×
//! and 4× CPU throttling. Needs `CHROME` and a Fieldnotes `host/web/dist`
//! (`bun host/web/build.mjs fieldnotes`; `EXACT_WEB_DIST` names another);
//! skips otherwise. The numbers are reported, not asserted: the bar is the
//! RFC's to judge, on a named device.
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn fieldnotes_web_drives_on_its_placement_and_reports_frame_gaps_under_a_backup() {
    let chrome = std::env::var("CHROME")
        .unwrap_or_else(|_| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !Path::new(&chrome).exists() {
        eprintln!("Fieldnotes web drive unavailable: set CHROME");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dist = std::env::var_os("EXACT_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("host/web/dist"));
    let receipt = std::fs::read_to_string(dist.join("app.module.json")).unwrap_or_default();
    if !receipt.contains("\"com.exact.fieldnotes\"") {
        eprintln!(
            "Fieldnotes web drive unavailable: {} is not a Fieldnotes build (bun host/web/build.mjs fieldnotes)",
            dist.display()
        );
        return;
    }
    let result = Command::new("bun")
        .args(["--input-type=module", "-e", PROBE])
        .env("CHROME", chrome)
        .env("EXACT_WEB_DIST", &dist)
        .current_dir(root)
        .output()
        .unwrap();
    eprintln!("{}", String::from_utf8_lossy(&result.stdout));
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

const PROBE: &str = r#"
import { Cdp } from './scripts/agent.mjs';
import { readStaticFile, webContentType } from './host/web/serve.mjs';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
const dist = process.env.EXACT_WEB_DIST;
const server = createServer((req, res) => {
  const found = readStaticFile(dist, req.url.split('?')[0]);
  if (!found) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
  res.end(found.body);
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const profile = mkdtempSync(resolve(tmpdir(), 'exact-fieldnotes-worker-'));
const child = spawn(process.env.CHROME, ['--headless=new', '--no-sandbox', '--remote-debugging-pipe', '--no-first-run', '--disable-background-networking', '--disable-extensions', `--user-data-dir=${profile}`, 'about:blank'], { detached: true, stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
const cdp = new Cdp(child.stdio[3], child.stdio[4]);
const exited = new Promise(r => child.on('exit', () => { cdp.fail('browser closed'); r(); }));
const logs = [];
try {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: targetInfos.find(t => t.type === 'page').targetId, flatten: true });
  const send = (method, params) => cdp.send(method, params, sessionId);
  cdp.listeners.push(m => { if (m.sessionId === sessionId && ['Runtime.exceptionThrown', 'Log.entryAdded'].includes(m.method)) logs.push(m); });
  await send('Page.enable'); await send('Runtime.enable'); await send('Log.enable');
  await send('Emulation.setDeviceMetricsOverride', { width: 1100, height: 1000, deviceScaleFactor: 1, mobile: false });
  const evaluate = async expression => { const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }); if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text); return r.result.value; };
  const frame = () => evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(()=>r(true))))');
  const busy = () => evaluate(`!!document.querySelector('[data-testid="working"]') || document.getElementById('exact-root')?.getAttribute('aria-busy')==='true'`);
  const settle = async () => { for (let i = 0; i < 600; i++) { await frame(); if (!(await busy())) return; } throw new Error('UI remained busy'); };
  const boot = async () => { for (let i = 0; i < 200; i++) { if (await evaluate("!!document.getElementById('exact-root')?.dataset.bootMs").catch(() => false)) { await evaluate('exact.ready'); await settle(); return; } await new Promise(r => setTimeout(r, 100)); } throw new Error('no first frame'); };
  const point = async testId => { const rect = await evaluate(`(()=>{const e=document.querySelector('[data-testid="'+${JSON.stringify(testId)}+'"]');if(!e)return null;e.scrollIntoView({block:'nearest'});const r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`); if (!rect) { const present = await evaluate(`[...document.querySelectorAll('[data-testid]')].map(e=>e.getAttribute('data-testid')).join(' ')`); assert.fail(`missing ${testId}; present: ${present}; state: ${JSON.stringify(await state())}; logs: ${JSON.stringify(logs.slice(-5))}`); } return rect; };
  const click = async testId => { const { x, y } = await point(testId); await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y }); await send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 }); await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 }); };
  const tap = async testId => { await click(testId); await settle(); };
  const type = async (testId, text) => { await tap(testId); await send('Input.insertText', { text }); await settle(); };
  const state = () => evaluate(`({revision:Number(localStorage.getItem('exact.secret.fieldnotes.revision')),count:document.querySelector('[data-testid="note-count"]')?.textContent,status:document.querySelector('[data-testid="status-message"]')?.textContent,backup:document.querySelector('[data-testid="backup-text"]')?.value?.length,notes:document.querySelectorAll('[data-testid^="note-"]').length})`);
  await send('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
  await boot();
  const placement = await evaluate('globalThis.exact.compat.inputs.typescriptPlacement');
  const rustPlacement = await evaluate('globalThis.exact.compat.inputs.rustPlacement');
  console.log(JSON.stringify({ placement, rustPlacement, boot: await state() }));
  // The notebook drive (LLP 1027.002 §5 step 4): save, back up, restore.
  await type('note-title', 'Field 🌿 東京');
  await type('note-body', 'Placed on ' + placement + '\nCafé, naïve, 🦀');
  await tap('save-note');
  const saved = await state();
  assert.equal(saved.revision, 1, JSON.stringify(saved));
  assert.match(saved.count, /^1 note/);
  await tap('backups'); await tap('save-backup');
  const backup = await state();
  assert.equal(backup.status, 'Backup saved with 1 notes.', JSON.stringify(backup));
  assert.equal(backup.revision, 2);
  // Seed 1,000 notes of 3,000 UTF-16 units through the app's own restore.
  const seeded = await evaluate(`(()=>{const notes=Array.from({length:1000},(_,i)=>({id:String(i+1),title:'Note '+(i+1),body:('x'.repeat(2999)+'\\n'),pinned:false}));const text=JSON.stringify({version:1,notes});const el=document.querySelector('[data-testid="import-text"]');el.focus();el.value=text;el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));return text.length;})()`);
  await settle();
  await tap('restore-backup'); await tap('confirm-restore');
  const restored = await state();
  assert.equal(restored.status, 'Restored 1000 notes.', JSON.stringify({ seeded, restored }));
  assert.match(restored.count, /^1000 notes/);
  // The frame-gap sampler (LLP 1027.002 §5 step 5): every animation frame's
  // distance from the previous one, on the page's main thread.
  await evaluate(`(()=>{const s=globalThis.__gaps={samples:[],last:performance.now(),on:true};const f=t=>{if(!s.on)return;s.samples.push(t-s.last);s.last=t;requestAnimationFrame(f);};requestAnimationFrame(f);})()`);
  const take = () => evaluate('(()=>{const s=globalThis.__gaps.samples.splice(0);return s;})()');
  const stats = (samples, frameMs) => { const sorted = [...samples].sort((a, b) => a - b); const q = p => sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] ?? NaN; return { frames: sorted.length, max_ms: sorted.at(-1) ?? NaN, p95_ms: q(0.95), p50_ms: q(0.5), dropped: samples.filter(g => g > 1.5 * frameMs).length }; };
  const report = [];
  for (const rate of [1, 4]) {
    await send('Emulation.setCPUThrottlingRate', { rate });
    await new Promise(r => setTimeout(r, 300)); await take();
    await new Promise(r => setTimeout(r, 700));
    const idle = await take();
    const frameMs = [...idle].sort((a, b) => a - b)[Math.floor(idle.length / 2)];
    // The Backups button toggles the panel: open it only when it is closed.
    if (!(await evaluate(`!!document.querySelector('[data-testid="backup-panel"]')`))) await tap('backups');
    await take();
    const started = Date.now();
    await click('save-backup');
    // Wheel the page while the backup runs, as a user would. The backup is
    // Rust in the page's wasm on either placement; the TypeScript library
    // refresh it causes is the turn that moves.
    const page = await point('fieldnotes');
    let status = '';
    while (Date.now() - started < 15000) {
      await send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: page.x, y: page.y, deltaX: 0, deltaY: 120 });
      status = await evaluate(`document.querySelector('[data-testid="status-message"]')?.textContent ?? ''`);
      if (/^Backup saved/.test(status) && !(await busy())) break;
    }
    const during = await take();
    const backupMs = Date.now() - started;
    assert.equal(status, 'Backup saved with 1000 notes.', status);
    await settle();
    // Typing into search: every keystroke is a TypeScript `library` turn
    // over the 1,000 notes — the turn that runs on the placed module.
    await tap('backups');
    await tap('find-notes');
    await take();
    const typingStarted = Date.now();
    for (const ch of 'quiet af') {
      await send('Input.insertText', { text: ch });
      await new Promise(r => setTimeout(r, 120));
    }
    await settle();
    const typing = await take();
    const typingMs = Date.now() - typingStarted;
    const matched = await evaluate(`document.querySelector('[data-testid="note-count"]')?.textContent`);
    await tap('close-search');
    report.push({ placement, cpu_throttle: rate, frame_ms: frameMs, refresh_hz: Math.round(1000 / frameMs), idle: stats(idle, frameMs), backup_ms: backupMs, during_backup: stats(during, frameMs), typing_ms: typingMs, during_typing: stats(typing, frameMs), matched });
  }
  await send('Emulation.setCPUThrottlingRate', { rate: 1 });
  await evaluate('globalThis.__gaps.on=false');
  console.log(JSON.stringify({ frame_gaps: report }));
  // A missing favicon is the page's only expected network error.
  const errors = logs.filter(m => m.method === 'Runtime.exceptionThrown' || (m.params?.entry?.level === 'error' && !/\/favicon\.ico$/.test(m.params.entry.url ?? '')));
  assert.equal(errors.length, 0, JSON.stringify(errors));
  console.log(`PASS Fieldnotes web on ${placement}: save / Rust backup / restore of 1000 notes / backup under scrolling at 1x and 4x CPU`);
} finally {
  try { process.kill(-child.pid, 'SIGKILL'); } catch {}
  await exited; server.closeAllConnections(); await new Promise(r => server.close(r)); rmSync(profile, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}
"#;
