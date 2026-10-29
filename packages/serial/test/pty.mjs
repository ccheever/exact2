// @exact/serial bound on macOS or the Linux host headless (LLP 1133.037 §16 stages 2 and 5, Bench drive 10): Bench with
// the Swift or the Rust implementation answering (`--bind serial`), the port a pseudo-terminal this script makes and names through D19 item 3's
// `EXACT_DEVICE_FIXTURE` (a development build's injected device). The pty is set raw and filled with a megabyte of
// numbered lines before Bench opens it; the writes block while nothing reads, so the bytes wait in the kernel as a
// device's would, and Bench must read every one, in order, through the module's termios, read loop and bound.
// A pty carries no modem lines, so signals are not checked here.
//
// usage: bun packages/serial/test/pty.mjs [macos|linux]
//   (after `bun host/apple/build.mjs bench-apple`, or `cargo build --profile host-dev -p bench-linux`)
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { checker, root } from './chromium.mjs';

const LINES = 8192, WIDTH = 128;
const host = process.argv[2] ?? 'macos';
const { check, failed } = checker(host);
const dir = mkdtempSync(join(tmpdir(), 'serial-pty-'));
// The pty: raw on its slave side (held open here, never read), the master written in full, then held until stdin ends.
const writer = spawn('python3', ['-I', '-c', `
import os, sys, tty, termios
master, slave = os.openpty()
tty.setraw(slave)
print(os.ttyname(slave), flush=True)
for i in range(${LINES}):
    line = ('line %05d ' % i).ljust(${WIDTH - 2}, '.') + '\\r\\n'
    os.write(master, line.encode())
sys.stdin.read()
`], { stdio: ['pipe', 'pipe', 'inherit'] });
const path = await new Promise((done) => writer.stdout.once('data', (d) => done(String(d).trim())));
const fixture = join(dir, 'devices.json');
writeFileSync(fixture, JSON.stringify({ serial: [{ id: 'pty', name: 'Fixture port', path }] }));

const ops = ['clock settle', 'state ports', 'type port pty', 'clock settle', 'tap connect', 'clock settle',
  ...Array.from({ length: 30 }, () => 'clock +500 real'), 'clock settle', 'state log', 'state conn'];
const r = spawnSync('bun', ['scripts/agent.mjs', host, '--json', '--app', 'bench', '--bind', 'serial', '--storage', `serial-pty-${Date.now()}`, '--reset-storage', ...ops],
  { cwd: root, encoding: 'utf8', env: { ...process.env, EXACT_DEVICE_FIXTURE: fixture }, maxBuffer: 1 << 28 });
writer.kill();
const replies = (r.stdout ?? '').split('\n').filter((l) => /^[[{"]/.test(l)).map((l) => JSON.parse(l));
if (replies.length < ops.length) console.log(r.stdout.slice(-2000), r.stderr.slice(-3000));
const value = (v) => v?.value ?? v;
const ports = value(replies[1]) ?? [];
check('the injected pty is listed beside the Mac\'s own ports', JSON.stringify(ports).includes('Fixture port'), JSON.stringify(ports).slice(0, 300));
const log = value(replies.at(-2)) ?? {}, conn = value(replies.at(-1)) ?? {};
const lines = log.lines ?? [];
const numbered = lines.map((l) => Number(/^line (\d{5}) /.exec(l.text)?.[1]));
check('the port opened through the module', conn.open === true, JSON.stringify(conn));
check(`all ${LINES} lines arrived (${LINES * WIDTH} bytes)`, log.total === LINES, `${log.total} lines kept`);
check('the lines shown are the last, in order, whole', numbered.length > 0 && numbered.at(-1) === LINES - 1 && numbered.every((n, i) => i === 0 || n === numbered[i - 1] + 1) && lines.every((l) => l.text.length === WIDTH - 2), JSON.stringify(lines.slice(-2)));
console.log(`${failed()} failed`);
process.exit(failed() ? 1 : 0);
