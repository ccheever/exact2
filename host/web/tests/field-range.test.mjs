import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
if (unavailable) console.warn(`SKIP: ${unavailable}`);
const check = unavailable ? (name, ...args) => test.skip(`${name} — ${unavailable}`, ...args) : test;

const page = `<button id="outside">Outside</button><div id="exact-root"></div>
<script type="module">
  import { commands } from './commands.js';
  import { runFocusCommands, controlEvent } from './navigation.js';
  import { onValue, OnHooks } from './rt.js';
  import './files.js';
  window.fileHook = OnHooks.file;
  import { ht } from './hatches.js';
  window.bindInput = (target, field, seen) => target === 'js'
    ? onValue(field, 'input', (value, record) => seen.push([value, record[5]]), (name, f) => field.addEventListener(name, f))
    : controlEvent(field, 'input', (name, f) => field.addEventListener(name, f), (kind) => seen.push([field.value, kind === 48 ? 'insertReplacementText' : '']));
  window.hatch = ht;
  window.logs = [];
  window.replace = (target, args) => target === 'js' ? commands(s => logs.push(s)).setRangeText(...args)
    : runFocusCommands([{ name: 'setRangeText', args }], { root: document.getElementById('exact-root'), ready: true, inertAncestor: () => false, log: s => logs.push(s) });
  window.ready = true;
</script>`;

/** The page in headless Chrome: `test(evaluate, call)` with its `Runtime.evaluate` and its CDP session. */
async function inChrome(test) {
  const server = createServer((req, res) => {
    if (req.url === '/favicon.ico') { res.writeHead(204); res.end(); return; }
    if (req.url === '/') { res.writeHead(200, { 'content-type': 'text/html' }); res.end(page); return; }
    res.writeHead(200, { 'content-type': 'text/javascript' });
    if (req.url === '/names.js') { res.end('export const sourceTypes = {};'); return; }
    if (req.url === '/native.js') { res.end('export const pageTable = () => ({ element: h => window.hatched(h) });'); return; }
    const js = resolve(WEB, '../web-js', req.url.slice(1));
    res.end(readFileSync(existsSync(js) ? js : resolve(WEB, req.url.replace(/^\/web\//, '/').slice(1))));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'exact-field-range-'));
  const child = spawn(chrome, [...(process.env.EXACT_HEADED_ORACLE ? [] : ['--headless=new']), '--remote-debugging-pipe', '--window-size=600,400', `--user-data-dir=${profile}`,
    '--no-sandbox', '--no-first-run', '--disable-background-networking', 'about:blank'], { stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
  try {
    const cdp = new Cdp(child.stdio[3], child.stdio[4]);
    const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const call = (method, params) => cdp.send(method, params, sessionId);
    const evaluate = async (expression) => {
      const reply = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
      return reply.result.value;
    };
    await call('Page.navigate', { url: `http://127.0.0.1:${server.address().port}/` });
    for (let i = 0; !(await evaluate('window.ready === true')); i++) { if (i > 400) throw new Error('page never ready'); await Bun.sleep(50); }
    await test(evaluate, call);
  } finally {
    child.kill();
    server.close();
    rmSync(profile, { recursive: true, force: true });
  }
}

// Compare the production command with HTML's method. Only the admitted input event differs.
check('setRangeText follows Chrome selection modes and emits one replacement input on both web targets', () => inChrome(async (evaluate) => {
  for (const target of ['js', 'wasm']) for (const tag of ['input', 'textarea']) {
    for (const args of [['[a.ts]'], ['@a.ts', 4, 9], ['[a.ts]', 4, 9], ['😀', 4, 9, 'select'], ['x', 0, 3, 'start'], ['x', 0, 3, 'end'], ['', 4, 9, 'preserve'], ['z', 100, 100], ['x', 0, 4, 'preserve'], ['a\nb', 4, 9]]) {
      const result = await evaluate(`(async () => {
        const root = document.getElementById('exact-root'); root.replaceChildren();
        const field = document.createElement(${JSON.stringify(tag)}), oracle = document.createElement(${JSON.stringify(tag)});
        field.id = 'editor'; root.append(field, oracle);
        for (const e of [field, oracle]) { e.value = 'see @a.ts now'; e.setSelectionRange(4, 9, 'backward'); }
        document.getElementById('outside').focus();
        await new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
        const seen = [], expected = [], input = [], before = [];
        field.addEventListener('beforeinput', e => before.push(e.inputType));
        for (const [e, events] of [[field, seen], [oracle, expected]]) for (const name of ['select', 'selectionchange']) e.addEventListener(name, () => events.push(name));
        field.addEventListener('input', e => input.push([e.inputType, e.data, e.bubbles, e.cancelable, e.isTrusted, field.value]));
        oracle.setRangeText(...${JSON.stringify(args)});
        replace(${JSON.stringify(target)}, ['editor', ...${JSON.stringify(args)}]);
        await new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
        const state = e => [e.value, e.selectionStart, e.selectionEnd, e.selectionDirection];
        return { actual: state(field), expected: state(oracle), seen, events: expected, input, before, focus: document.activeElement.id };
      })()`);
      expect(result.actual).toEqual(result.expected);
      expect(result.seen).toEqual(result.events);
      expect(result.input).toEqual([['insertReplacementText', args[0], true, false, false, result.actual[0]]]);
      expect(result.focus).toBe('outside');
      expect(result.before).toEqual([]);
    }
    const refused = await evaluate(`(() => {
      const field = document.getElementById('editor'); let inputs = 0;
      field.addEventListener('input', () => inputs++);
      const old = field.value; logs.length = 0;
      for (const args of [['editor', 'x', 9, 4], ['editor', 'x', 0, 1, 'invalid'], ['missing', 'x'], ['outside', 'x']]) replace(${JSON.stringify(target)}, args);
      return [field.value === old, inputs, logs.length];
    })()`);
    expect(refused).toEqual([true, 0, 4]);
  }
  expect(await evaluate(`(() => {
    const root = document.getElementById('exact-root'); root.innerHTML = '<input id="number" type="number" value="12">';
    logs.length = 0; replace('js', ['number', 'x', 0, 1]); return [document.getElementById('number').value, logs.length];
  })()`)).toEqual(['12', 1]);
  expect(await evaluate(`(() => {
    const root = document.getElementById('exact-root'); root.innerHTML = '<textarea id="editor" markup="markdown">before</textarea>';
    logs.length = 0; replace('js', ['editor', 'after', 0, 6]); return [document.getElementById('editor').value, logs.length];
  })()`)).toEqual(['before', 1]);
  // No native undo transaction is introduced by the command.
  expect(await evaluate(`(() => {
    const root = document.getElementById('exact-root'); root.innerHTML = '<textarea id="editor">before</textarea>';
    const field = document.getElementById('editor'); field.focus(); replace('js', ['editor', 'after', 0, 6]);
    document.execCommand('undo'); return field.value;
  })()`)).toBe('after');
}), 30000);

check('range edit event order and URL sanitization match Chrome', () => inChrome(async (evaluate) => {
  for (const target of ['js', 'wasm']) {
    for (const tag of ['input', 'textarea']) for (const [value, start, end, args, expected] of [
      ['hello', 0, 0, ['!', 2, 2], ['input']],
      ['hello', 0, 0, ['!', 2, 2, 'end'], ['input', 'select']],
      ['see @a.ts now', 4, 9, ['[a.ts]', 4, 9], ['input', 'select']],
    ]) {
      expect(await evaluate(`(async () => {
        const root = document.getElementById('exact-root'); root.replaceChildren();
        const field = document.createElement(${JSON.stringify(tag)}); field.id = 'editor'; root.append(field);
        field.value = ${JSON.stringify(value)};
        await new Promise(ok => { field.addEventListener('select', ok, { once: true }); field.setSelectionRange(${start}, ${end}); });
        const events = []; for (const name of ['input', 'select']) field.addEventListener(name, () => events.push(name));
        const selected = ${expected.length} === 2 ? new Promise(ok => field.addEventListener('select', ok, { once: true })) : new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
        replace(${JSON.stringify(target)}, ['editor', ...${JSON.stringify(args)}]);
        const immediate = [...events]; await selected;
        return { immediate, events };
      })()`)).toEqual({ immediate: ['input'], events: expected });
    }
    for (const [raw, expected] of [[' \t\nhttps://example.com\r\f ', 'https://example.com'], ['\u00a0x\u00a0', '\u00a0x\u00a0']]) expect(await evaluate(`(() => {
      const root = document.getElementById('exact-root'); root.innerHTML = '<input id="url" type="url">';
      const field = document.getElementById('url'), inputs = [];
      field.addEventListener('input', () => inputs.push(field.value));
      replace(${JSON.stringify(target)}, ['url', ${JSON.stringify(raw)}, 0, 0, 'end']);
      return [field.value, field.selectionStart, inputs];
    })()`)).toEqual([expected, expected.length, [expected]]);
  }
}), 30000);

check('web replacement input types include the hatch and browser correction events', () => inChrome(async (evaluate) => {
  for (const target of ['js', 'wasm']) {
    expect(await evaluate(`(async () => {
      const root = document.getElementById('exact-root'); root.innerHTML = '<input id="editor" data-hatch="editor">';
      const field = document.getElementById('editor'), seen = [];
      bindInput(${JSON.stringify(target)}, field, seen);
      replace(${JSON.stringify(target)}, ['editor', 'x', 0, 0, 'end']);
      const supplied = new Promise(ok => { window.hatched = h => { field.addEventListener('input', ok, { once: true }); h.input('Palo'); }; });
      hatch(field); await supplied;
      // A browser spelling correction's DOM event is also insertReplacementText.
      field.value = 'corrected'; field.dispatchEvent(new InputEvent('input', { inputType: 'insertReplacementText', bubbles: true }));
      field.value = 'typed'; field.dispatchEvent(new InputEvent('input', { inputType: 'insertText', bubbles: true }));
      return seen;
    })()`)).toEqual([['x', 'insertReplacementText'], ['Palo', 'insertReplacementText'], ['corrected', 'insertReplacementText'], ['typed', '']]);
  }
}), 30000);

check('replacement select tasks read the final value and reset directional ranges', () => inChrome(async evaluate => {
  for (const target of ['js', 'wasm']) for (const tag of ['input', 'textarea']) {
    expect(await evaluate(`(async () => {
      const root = document.getElementById('exact-root'); root.replaceChildren();
      const field = document.createElement(${JSON.stringify(tag)}); field.id = 'editor'; root.append(field);
      const events = [];
      field.addEventListener('input', () => events.push('input:' + field.value));
      field.addEventListener('select', () => events.push('select:' + field.value));
      const selected = new Promise(ok => { let n = 0; field.addEventListener('select', () => { if (++n === 2) ok(); }); });
      replace(${JSON.stringify(target)}, ['editor', 'A', 0, 0, 'end']);
      replace(${JSON.stringify(target)}, ['editor', 'B', 1, 1, 'end']);
      const immediate = [...events]; await selected;
      return { immediate, events };
    })()`)).toEqual({ immediate: ['input:A', 'input:AB'], events: ['input:A', 'input:AB', 'select:AB', 'select:AB'] });
    for (const focused of [false, true]) for (const [start, end, direction, args, offsets] of [
      [0, 2, 'forward', ['!', 4, 4], [0, 2]],
      [1, 4, 'backward', ['XY', 2, 3], [1, 5]],
    ]) {
      const result = await evaluate(`(async () => {
        const field = document.getElementById('editor'), oracle = document.createElement('${tag}');
        field.after(oracle); const events = [], expected = [];
        for (const [e, log] of [[field, events], [oracle, expected]]) {
          e.value = 'hello'; if (${focused}) e.focus(); else document.getElementById('outside').focus();
          await new Promise(ok => { e.addEventListener('select', ok, { once: true }); e.setSelectionRange(${start}, ${end}, '${direction}'); });
          e.addEventListener('select', () => log.push('select'));
        }
        if (${focused}) field.focus();
        replace('${target}', ['editor', ...${JSON.stringify(args)}]);
        if (${focused}) oracle.focus();
        oracle.setRangeText(...${JSON.stringify(args)});
        await new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
        const state = e => [e.selectionStart, e.selectionEnd, e.selectionDirection];
        const result = { actual: [...state(field), events.length], oracle: [...state(oracle), expected.length] };
        oracle.remove(); return result;
      })()`);
      expect(result.actual).toEqual(result.oracle);
      if (process.platform === 'darwin') expect(result.actual).toEqual([...offsets, 'none', 1]);
    }
  }
}), 30000);

check('file picker change supplies the complete InputEvent record', () => inChrome(async evaluate => {
  expect(await evaluate(`(() => {
    const field = document.createElement('input'); field.type = 'file';
    let record, branch;
    fileHook(field, 'change', (files, e) => { record = e; branch = e[5] === ''; });
    field.dispatchEvent(new CustomEvent('exact-picked', { detail: 'app:/tmp/picked/a.txt\tfile.txt\ttext/plain\t1\t\t\t' }));
    return [record, branch];
  })()`)).toEqual([['', false, 0, 0, 'none', ''], true]);
}));
