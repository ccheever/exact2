// LLP 1133.037 §12.2's Bench drives under the agent's substitute: the serial monitor (drives 1–3, 5–7) and the
// DFU updater (drive 8). Drive 9 is the runner's test (a tool's open), Bench declaring no tool. Drive 4 needs the sample host's two sessions, 10 and 11 a pseudo-terminal or a Linux kernel
// under a bound implementation, 12 a person with boards: none of them is here.
//
// usage: bun apps/bench/bench.drives.mjs <web|macos|linux|ios> [agent flags]
//   EXACT_DRIVE_DEBUG=1 prints every reply
//   (the app built for that host: host/web/build.mjs bench, host/apple/build.mjs [--ios] bench-apple, or
//   cargo build --profile host-dev -p bench-linux)
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';

const root = resolve(import.meta.dir, '../..');
const host = process.argv[2] ?? 'web', extra = process.argv.slice(3);
let failures = 0;
const check = (what, ok, detail = '') => { if (!ok) failures++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${host} ${what}${ok ? '' : `: ${detail}`}`); };

/** One launch: each op's JSON reply, in order. */
function drive(storage, ops, { keep = false } = {}) {
  const args = ['scripts/agent.mjs', host, '--json', '--app', 'bench', '--storage', storage, ...(keep ? [] : ['--reset-storage']), ...extra, ...ops];
  const r = spawnSync('bun', args, { cwd: root, encoding: 'utf8', maxBuffer: 1 << 26 });
  const replies = (r.stdout ?? '').split('\n').filter(l => /^[[{"]|^(null|true|false|-?\d)/.test(l)).map(l => JSON.parse(l));
  if (replies.length < ops.length || process.env.EXACT_DRIVE_DEBUG) console.log(ops.map((o, i) => `${i} ${o.slice(0, 60)} → ${JSON.stringify(replies[i])?.slice(0, 400)}`).join('\n'), r.stderr.slice(-3000));
  return replies;
}
const text = v => JSON.stringify(v) ?? '';
const b64 = s => Buffer.from(s, 'utf8').toString('base64');
/** `ms` of the agent clock in 50 ms steps, each settled: a native host's data code answers on its own time. */
const wait = ms => Array.from({ length: Math.ceil(ms / 50) }, () => ['clock +50', 'clock settle']).flat();
const prefer = (key, value) => `prefer extension ${key}=${JSON.stringify(value)}`;
const ticket = pending => (Array.isArray(pending) ? pending : []).find(p => p.device)?.ticket;
const run = Date.now().toString(36);
const native = host !== 'web';

// Drive 1's port: a boot log, then a line whose "é" is split between two events, then a break.
const port = (connected = true) => [{
  id: 's3', name: 'USB JTAG/serial debug unit', usbVendorId: 12346, usbProductId: 4097, connected,
  input: [
    { at: 0, data: b64('ESP-ROM:esp32s3-20210327\r\nboot:0x8 (SPI_FAST_FLASH_BOOT)\r\n') },
    { at: 200, data: Buffer.from([0x63, 0x61, 0x66, 0xc3]).toString('base64') },
    { at: 250, data: Buffer.from([0xa9, 0x0d, 0x0a]).toString('base64') },
    { at: 600, error: 'BreakError' },
    { at: 700, data: b64('after the break\r\n') },
  ],
}];

if (host === 'ios') {
  const r = drive(`bench-ios-${run}`, ['clock settle', 'tree']);
  check('the status reads unavailable: no serial ports', text(r[1]).includes('no-serial'), text(r[1]).slice(0, 400));
} else {
  // 1–3, 5, 6: native lists the port; web chooses it (7). Each reply is found by its op's label.
  const steps = [['settle', 'clock settle'], ['prefer', prefer('serial.ports', port())], ['', 'clock settle'],
    ...(native ? [['pick', 'type port s3']] : [['choose', 'tap choose-port'], ['', 'clock +50'], ['pending', 'state pending'], ['chosen', 'tap @chooser s3']]),
    ['', 'clock settle'], ['', 'tap connect'], ['', 'clock settle'], ...wait(150).map(o => ['', o]), ['boot', 'tree log'],
    ...wait(250).map(o => ['', o]), ['split', 'tree log'], ['', 'type send help'], ['', 'type send key Enter'], ['', 'clock settle'],
    ['', 'tap reset-board'], ...wait(150).map(o => ['', o]), ...wait(600).map(o => ['', o]), ['later', 'tree log'],
    ['state', 'state extensions.serial'], ['', prefer('serial.ports', port(false))], ['', 'clock +200'], ['', 'clock settle'], ['gone', 'tree']];
  const r = drive(`bench-1-${run}`, steps.map(([, op]) => op));
  const at = label => r[steps.findIndex(([l]) => l === label)];
  if (!native) check('7: Choose Port… holds a chooser ticket', text(at('pending')).includes('"chooser"'), text(at('pending')));
  check('1: the log shows the boot lines', text(at('boot')).includes('ESP-ROM:esp32s3-20210327') && text(at('boot')).includes('boot:0x8'), text(at('boot')).slice(0, 600));
  check('1: the split line arrives whole', text(at('split')).includes('café'), text(at('split')).slice(0, 900));
  check('6: a break is marked and the log goes on', text(at('later')).includes('— break —') && text(at('later')).includes('after the break'), text(at('later')).slice(0, 900));
  const state = at('state'), writes = state?.writes ?? state?.value?.writes ?? [];
  check('2: Return writes the line with CR LF', text(writes).includes(b64('help\r\n')), text(writes));
  check('3: Reset Board asserts RTS, then releases it 100 ms later', writes.filter(w => w.name === 'setSignals').map(w => w.requestToSend).join() === 'true,false', text(writes));
  check('5: unplugged reads Disconnected', text(at('gone')).includes('disconnected'), text(at('gone')).slice(0, 600));
  if (!native) {
    // 7: a reload lists the port from getPorts(), with no ticket (the agent's preferred values are the page's, so
    // the port is preferred again).
    const again = drive(`bench-7-${run}`, ['clock settle', prefer('serial.ports', port()), 'clock settle', 'tap choose-port', 'clock +50', 'tap @chooser s3', 'clock settle',
      'reload', 'clock settle', prefer('serial.ports', port()), 'clock settle', 'tree', 'state pending']);
    check('7: after a reload the port is remembered (getPorts), no ticket', ticket(again.at(-1)) == null && text(again.at(-2)).includes('chosen-port'), text(again.at(-2)).match(/chosen-port[^}]*/)?.[0] ?? text(again.at(-2)).slice(0, 400));
  }
  // 9, a tool's open refused `person-required`, is the runner's (contract/cli/tests/it/devices.rs): Bench declares no
  // tool for a drive to call.

  // 8: the DFU updater over the GET_STATUS list.
  const status = (state, code = 0) => ({ status: 'ok', data: Buffer.from([code, 0, 0, 0, state, 0]).toString('base64') });
  const blocks = n => Array.from({ length: n }, () => [status(4), status(5)]).flat();
  const board = (list) => [{
    id: 'stm', vendorId: 1155, productId: 57105, productName: 'STM32 BOOTLOADER',
    configurations: [{ configurationValue: 1, interfaces: [{ interfaceNumber: 0, alternates: [{ alternateSetting: 0, interfaceClass: 254, interfaceSubclass: 1, interfaceProtocol: 2, interfaceName: '@Internal Flash  /0x08000000/04*016Kg', endpoints: [] }] }] }],
    control: { 'in class interface 3 0 0': list },
  }];
  // The image: 2,500 bytes, 3 blocks of 1,024; DfuSe erases one 16 KB page and sets the address first.
  const ok = [status(2), ...blocks(2), ...blocks(3), status(7), status(2)];
  const image = resolve(tmpdir(), `bench-image-${run}.bin`);
  await Bun.write(image, new Uint8Array(2500).fill(0x5a));
  const pick = native ? ['type board stm'] : ['tap choose-board', 'clock +50', 'state pending', 'tap @chooser stm'];
  const u = drive(`bench-8-${run}`, ['clock settle', prefer('usb.devices', board(ok)), 'tap tab-update', 'clock settle', ...pick, 'clock settle',
    'tap choose-file', 'clock +50', `pick image-input ${image}`, 'clock settle', 'tap update', ...wait(1500), 'tree', 'state extensions.usb']);
  const tree = u.at(-2), usb = u.at(-1), sent = usb?.writes ?? usb?.value?.writes ?? [];
  const outs = sent.filter(w => w.name === 'controlTransferOut');
  check('8: the update ends at 100 %', text(tree).includes('100 %') && text(tree).includes('updated'), text(tree).slice(0, 800));
  check('8: DfuSe erase and set-address, then the blocks, then the zero-length end', outs.length === 6 && outs[0].data?.startsWith('QQ') && outs[1].data?.startsWith('IQ') && !outs.at(-1).data, text(outs.map(o => [o.setup?.value, (o.data ?? '').slice(0, 4)])));
  const failed = [status(2), ...blocks(2), ...blocks(2), status(4), status(10, 7)];
  const f = drive(`bench-8b-${run}`, ['clock settle', prefer('usb.devices', board(failed)), 'tap tab-update', 'clock settle', ...pick, 'clock settle',
    'tap choose-file', 'clock +50', `pick image-input ${image}`, 'clock settle', 'tap update', ...wait(1500), 'tree', 'state extensions.usb']);
  const fw = f.at(-1)?.writes ?? f.at(-1)?.value?.writes ?? [];
  check('8: errVERIFY after the third block says the update failed', text(f.at(-2)).includes('update-error') && text(f.at(-2)).includes('errVERIFY'), text(f.at(-2)).slice(0, 800));
  check('8: and clears the status', fw.some(w => w.name === 'controlTransferOut' && w.setup?.request === 4), text(fw.map(w => w.setup?.request)));
}
console.log(`${failures} failed`);
process.exit(failures ? 1 : 0);
