import { test, expect } from 'bun:test';
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { chromium } from '../../../scripts/agent-launch.mjs';
import { need } from '../../../scripts/coverage.mjs';

const WEB = resolve(new URL('..', import.meta.url).pathname);
const { executable: chrome, unavailable } = chromium();
const check = need('chrome', unavailable);

const page = `<button id="outside">Outside</button><div id="exact-root"></div>
<script type="module">
  import { commands } from './commands.js';
  import { runFocusCommands, controlEvent, onSelection } from './navigation.js';
  import { onValue, OnHooks, onSelectionChange } from './rt.js';
  import * as rt from './rt.js';
  window.rt = rt;
  window.bindSelection = field => onSelectionChange(field, 'selectionchange', record => window.seen.push(record), (name, f) => field.addEventListener(name, f));
  import './files.js';
  window.fileHook = OnHooks.file;
  import { ht } from './hatches.js';
  window.bindInput = (target, field, seen) => target === 'js'
    ? onValue(field, 'input', (value, record) => seen.push([value, record[5]]), (name, f) => field.addEventListener(name, f))
    : controlEvent(field, 'input', (name, f) => field.addEventListener(name, f), (kind, payload) => seen.push([field.value, kind === 48 ? 'insertReplacementText' : kind === 54 ? payload.split(',')[0] : '']));
  window.hatch = ht;
  window.logs = [];
  window.replace = (target, args) => target === 'js' ? commands(s => logs.push(s)).setRangeText(...args)
    : runFocusCommands([{ name: 'setRangeText', args }], { root: document.getElementById('exact-root'), ready: true, inertAncestor: () => false, log: s => logs.push(s) });
  window.onSelection = onSelection;
  window.selectionCommand = (target, name, args = []) => target === 'js' ? commands(s => logs.push(s))[name](...args)
    : runFocusCommands([{ name, args }], { root: document.getElementById('exact-root'), ready: true, inertAncestor: () => false, log: s => logs.push(s) });
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
    })()`)).toEqual([['x', 'insertReplacementText'], ['Palo', 'insertReplacementText'], ['corrected', 'insertReplacementText'], ['typed', 'insertText']]);
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
  })()`)).toEqual([['', false, 0, 0, 'none', '', null, false, false], true]);
}));

check('text selection commands and rectangles match Chrome on both web targets', () => inChrome(async evaluate => {
  for (const target of ['js', 'wasm']) {
    const result = await evaluate(`(async () => {
      const root = document.getElementById('exact-root');
      root.innerHTML = '<div data-exact-text id="p1" style="width:120px;font:16px Arial;line-height:24px">Alpha beta <b>gamma delta</b> epsilon 😀 zeta.</div><div data-exact-text id="p2">Second paragraph</div>';
      const p = document.getElementById('p1'), q = document.getElementById('p2'), heard = [];
      onSelection(p, (...args) => heard.push(['p1', ...args]));
      onSelection(q, (...args) => heard.push(['p2', ...args]));
      const settled = () => new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
      const range = document.createRange(); range.setStart(p.firstChild, 6); range.setEnd(q.firstChild, 6);
      getSelection().removeAllRanges(); getSelection().addRange(range); await settled();
      const drag = heard.slice();
      const part = document.createRange(); part.setStart(p.firstChild, 6); part.setEnd(p.lastChild, p.lastChild.length);
      const oracle = [...part.getClientRects()].map(r => [r.x, r.y, r.width, r.height, false, false]);
      selectionCommand('${target}', 'removeAllRanges'); await settled();
      const cleared = heard.slice(drag.length);
      selectionCommand('${target}', 'setSelectionRange', ['p1', 0, 5, 'backward']); await settled();
      const selected = heard.at(-1), direction = [getSelection().anchorOffset, getSelection().focusOffset];
      selectionCommand('${target}', 'setSelectionRange', ['p1', 31, 33]); await settled();
      const emoji = heard.at(-1);
      selectionCommand('${target}', 'removeAllRanges'); await settled();
      return { drag, oracle, cleared, selected, direction, emoji, logs };
    })()`);
    expect(result.drag[0].slice(0, 4)).toEqual(['p1', 'beta gamma delta epsilon 😀 zeta.', 6, 39]);
    expect(result.drag[0][4]).toEqual(result.oracle);
    expect(result.drag[0][4].length).toBeGreaterThan(1);
    expect(result.cleared).toEqual([['p1', '', 0, 0, []], ['p2', '', 0, 0, []]]);
    expect(result.selected.slice(0, 4)).toEqual(['p1', 'Alpha', 0, 5]);
    expect(result.selected[4][0][2]).toBeGreaterThan(0);
    expect(result.direction).toEqual([5, 0]);
    expect(result.logs).toEqual([]);
    expect(result.emoji.slice(0, 4)).toEqual(['p1', '😀', 31, 33]);
  }
}), 30000);

check('a mouse drag reports viewport rectangles across paragraphs', () => inChrome(async (evaluate, call) => {
  const [from, to] = await evaluate(`(() => {
    const root = document.getElementById('exact-root');
    root.innerHTML = '<div data-exact-text id="p1" style="width:180px;font:18px Arial;line-height:24px">Alpha beta gamma delta epsilon 😀 zeta.</div><div data-exact-text id="p2" style="font:18px Arial;line-height:24px">Second paragraph</div>';
    window.heard = [];
    for (const id of ['p1', 'p2']) onSelection(document.getElementById(id), (...args) => heard.push([id, ...args]));
    return ['p1', 'p2'].map(id => { const r = document.getElementById(id).getBoundingClientRect(); return [r.x + 30, r.y + 12]; });
  })()`);
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: from[0], y: from[1], button: 'left', buttons: 1, clickCount: 1 });
  await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x: to[0], y: to[1], button: 'left', buttons: 1 });
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: to[0], y: to[1], button: 'left', buttons: 0, clickCount: 1 });
  const result = await evaluate(`(async () => {
    await new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
    const part = heard.findLast(e => e[0] === 'p2'), p = document.getElementById('p2'), range = document.createRange();
    range.setStart(p.firstChild, part[2]); range.setEnd(p.firstChild, part[3]);
    return { part, bottom: [...range.getClientRects()].at(-1).bottom, first: heard.findLast(e => e[0] === 'p1') };
  })()`);
  expect(result.first[4].length).toBeGreaterThan(1);
  const last = result.part[4].at(-1);
  expect(Math.abs(last[1] + last[3] - result.bottom)).toBeLessThanOrEqual(1);
}), 30000);

check('text source ranges include breaks, inline ids, collapsed spaces and column placement', () => inChrome(async evaluate => {
  for (const target of ['js', 'wasm']) {
    const result = await evaluate(`(async () => {
      const root = document.getElementById('exact-root');
      root.innerHTML = '<div data-exact-text id="p">Hello<br><b>world</b></div><div data-exact-text id="inline">Before <span id="word" data-exact-text>Alpha</span> after</div><div data-exact-text id="spaces">a  b</div><div data-exact-text id="columns" style="position:absolute;left:8px;top:29px;width:220px;height:48px;columns:2;column-gap:20px;column-fill:auto;white-space:pre;font:16px Arial;line-height:24px">Alpha\\nbeta\\ngamma\\ndelta</div>';
      const heard = {}, byId = id => document.getElementById(id);
      for (const id of ['p', 'inline', 'word', 'spaces', 'columns']) onSelection(byId(id), (...args) => heard[id] = args);
      const settled = () => new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(ok)));
      const rects = r => [...r.getClientRects()].map(r => [r.x, r.y, r.width, r.height, false, false]);
      const range = document.createRange(); range.selectNodeContents(byId('p').lastChild);
      getSelection().removeAllRanges(); getSelection().addRange(range); await settled();
      const dragged = heard.p, nativePrefix = (() => { const r = document.createRange(); r.setStart(byId('p'), 0); r.setEnd(byId('p').lastChild.firstChild, 0); return r.toString(); })();
      selectionCommand('${target}', 'removeAllRanges'); await settled();
      selectionCommand('${target}', 'setSelectionRange', ['p', 6, 11]); await settled();
      const hardBreak = [heard.p, getSelection().toString(), rects(range)];
      selectionCommand('${target}', 'setSelectionRange', ['word', 0, 99, 'backward']); await settled();
      const inline = [heard.inline, heard.word, getSelection().toString()];
      const spaces = [];
      for (const [a, b] of [[0, 2], [0, 3], [2, 3]]) {
        selectionCommand('${target}', 'setSelectionRange', ['spaces', a, b]); await settled(); spaces.push(heard.spaces);
      }
      selectionCommand('${target}', 'setSelectionRange', ['columns', 11, 16]); await settled();
      range.setStart(byId('columns').firstChild, 11); range.setEnd(byId('columns').firstChild, 16);
      return { dragged, nativePrefix, hardBreak, inline, spaces, columns: heard.columns, columnOracle: rects(range), logs };
    })()`);
    // Range.toString omits <br>; source offsets deliberately count its newline.
    expect(result.nativePrefix).toBe('Hello');
    expect(result.dragged.slice(0, 3)).toEqual(['world', 6, 11]);
    expect(result.hardBreak[0].slice(0, 3)).toEqual(['world', 6, 11]);
    expect(result.hardBreak[1]).toBe('world');
    expect(result.hardBreak[0][3]).toEqual(result.hardBreak[2]);
    expect(result.inline[0].slice(0, 3)).toEqual(['Alpha', 7, 12]);
    expect(result.inline[1].slice(0, 3)).toEqual(['Alpha', 0, 5]);
    expect(result.inline[2]).toBe('Alpha');
    expect(result.spaces.map(s => s.slice(0, 3))).toEqual([['a ', 0, 2], ['a  ', 0, 3], [' ', 2, 3]]);
    expect(result.columns[3]).toEqual(result.columnOracle);
    expect(result.columnOracle[0][0]).toBe(128);
    expect(result.columnOracle[0][1]).toBe(32);
    expect(result.columnOracle[0][3]).toBe(17);
    expect(result.logs).toEqual([]);
  }
}), 30000);

check('field selectionchange hears collapsed caret moves through the production binder', () => inChrome(async (evaluate, call) => {
  await evaluate(`(() => {
    const field = document.createElement('textarea'); field.value = 'see @a.ts now';
    document.getElementById('exact-root').append(field); field.focus(); field.setSelectionRange(4, 9);
    window.seen = []; bindSelection(field);
  })()`);
  for (let i = 0; i < 2; i++) {
    await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37 });
    await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowLeft', code: 'ArrowLeft', windowsVirtualKeyCode: 37 });
  }
  const seen = await evaluate(`new Promise(ok => requestAnimationFrame(() => requestAnimationFrame(() => ok(window.seen))))`);
  expect(seen.at(-1)?.slice(0, 5)).toEqual(['see @a.ts now', false, 3, 3, 'none']);
}));

check('beforeinput can cancel a real insertion but cannot cancel composition', () => inChrome(async (evaluate, call) => {
  expect(await evaluate('typeof rt.onBeforeInput')).toBe('function');
  await evaluate(`(() => {
    const field = document.createElement('textarea'); field.id = 'editing'; field.value = 'ab';
    document.getElementById('exact-root').append(field); field.focus(); field.setSelectionRange(1, 1);
    window.seen = [];
    rt.onBeforeInput(field, 'beforeinput', e => { seen.push(e); rt.Hosts.preventDefault(); }, (name, f) => field.addEventListener(name, f));
  })()`);
  await call('Input.insertText', { text: 'X' });
  expect(await evaluate('document.getElementById("editing").value')).toBe('ab');
  expect(await evaluate('seen[0].slice(5)')).toEqual(['insertText', 'X', true, false]);
  expect(await evaluate(`(() => {
    const event = new InputEvent('beforeinput', { inputType: 'insertCompositionText', data: 'あ', isComposing: true, cancelable: false });
    document.getElementById('editing').dispatchEvent(event); return [event.defaultPrevented, seen.at(-1).slice(5)];
  })()`)).toEqual([false, ['insertCompositionText', 'あ', false, true]]);
}));

check('Chrome paste cancellation and undo report their real input types', () => inChrome(async (evaluate, call) => {
  const origin = await evaluate('location.origin');
  await call('Browser.grantPermissions', { origin, permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] });
  await evaluate(`(async () => {
    const field = document.createElement('textarea'); field.id = 'history'; field.value = 'ab';
    document.getElementById('exact-root').append(field); field.focus(); field.setSelectionRange(1, 1);
    window.seen = [];
    rt.onBeforeInput(field, 'beforeinput', e => { seen.push(['before', ...e.slice(5)]); if (e[5] === 'insertFromPaste') rt.Hosts.preventDefault(); }, (name, f) => field.addEventListener(name, f));
    rt.onValue(field, 'input', (v, e) => seen.push(['input', ...e.slice(5)]), (name, f) => field.addEventListener(name, f));
    await navigator.clipboard.writeText('PASTE');
  })()`);
  await call('Input.insertText', { text: 'X' });
  await call('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'v', code: 'KeyV', modifiers: 4, commands: ['paste'] });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'v', code: 'KeyV' });
  expect(await evaluate('document.getElementById("history").value')).toBe('aXb');
  expect(await evaluate('seen.at(-1)')).toEqual(['before', 'insertFromPaste', 'PASTE', true, false]);
  await call('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: 'z', code: 'KeyZ', modifiers: 4, commands: ['undo'] });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'z', code: 'KeyZ' });
  expect(await evaluate('document.getElementById("history").value')).toBe('ab');
  expect(await evaluate('seen.slice(-2)')).toEqual([
    ['before', 'historyUndo', null, true, false], ['input', 'historyUndo', null, false, false],
  ]);
}));
