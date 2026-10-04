// Files on the JS target: a file input and `showPicker` (LLP 1069.002),
// `saveFile` and the three document pickers (LLP 1069.010 D2, D3) and
// `share` (LLP 1069.003) — the runner's rulings (runner/src/runner/
// picker.rs, save_file.rs, file_pickers.rs, share.rs: the checks, the
// refusals into the journal, the agent's holds) over the web host's own
// `picker-glue.js` and `documents-glue.js`, fetched on first use. The commands run in
// the press's commit, so the element's own picker opens inside the
// activation. Under the agent each request is held (`pick`, `export`,
// `open-file`, `open-directory`, `save-file`, `share`, LLP 1069.007 D3) and
// `tap @t …` / `type @t …` answers it
// (agent.js). Bundled only for a plan with a file input or one of the
// commands.
import { journal, clock, nextTicket, inflight, Hosts, OnHooks, viewId, Views, data } from './rt.js';
import { record } from './pointer.js';
import { reportPlace } from './navigation.js';
import { appGrantSet, coversPath } from './admission.js';

const say = line => journal.push(`t=${clock.now} ${line}`);
const x = () => (globalThis.exact ??= {});

// The web host's glue, as glue.js installs it: `dispatch(view, kind,
// payload)` in the runner's host event kinds — 26 a file input's picked
// files, 1 `change` with a string, 27 HTML's `cancel`.
let Glue = null, picked = 0;
const glue = () => Glue ??= import(new URL('./picker-glue.js', import.meta.url).href).then(() => x().picker({
  appId: data.appId, log: say,
  pickedPath: async name => `app:/tmp/picked/${reportPlace().split('\0')[2]}-${++picked}.${extension(name)}`,
  dispatch(view, kind, payload) {
    const el = Views.get(view);
    if (!el) return;
    if (kind === 26) el.dispatchEvent(new CustomEvent('exact-picked', { detail: payload }));
    else if (kind === 27) el.dispatchEvent(new Event('cancel'));
    else el.dispatchEvent(new CustomEvent('change', { detail: payload }));
  },
}));
const counted = p => { inflight.n++; return Promise.resolve(p).finally(() => inflight.n--); };
const extension = name => { const e = /\.([^.]*)$/.exec(name)?.[1]?.toLowerCase() ?? ''; return e && e.length <= 8 && /^[a-z0-9]+$/.test(e) ? e : 'bin'; };
const byId = id => { const el = document.getElementById(id); return el ? { el, view: viewId(el) } : null; };

// A file input's `change` payload (picker.rs `Picked::payload`): one line
// per file, as `Picked` records; null for anything malformed.
function records(payload) {
  const number = s => { const n = Number(s); return s !== '' && Number.isFinite(n) && n >= 0 ? n : undefined; };
  const out = [];
  for (const line of payload.split('\n').filter(Boolean)) {
    const [path, name, type, size, ...media] = line.split('\t');
    const rest = path?.startsWith('app:/tmp/picked/') ? path.slice(16) : '';
    if (media.length !== 3 || !rest || rest.includes('/') || rest.includes('..') || number(size) === undefined) return null;
    const maybe = media.map(s => s === '' ? null : number(s));
    if (maybe.includes(undefined)) return null;
    out.push([path, name, type, number(size), ...maybe]);
  }
  return out;
}
// A file input: `change` is the person's choice, read by the glue into
// `Picked` records; the element's own `cancel` is the DOM's.
OnHooks.file = (e, kind, f) => {
  if (kind !== 'change') return false;
  e.addEventListener('change', ev => { if (ev.isTrusted) counted(glue().then(m => m.change(e, viewId(e)))); });
  e.addEventListener('exact-picked', ev => {
    const files = records(ev.detail);
    if (!files) return say('picker: refused: a malformed payload');
    f(files);
  });
  return true;
};

// ---------------------------------------------------------------- holds (LLP 1069.007 D3)
const Holds = [];
function hold(capability, el, args, answer) {
  const ticket = nextTicket();
  const name = el ? el.getAttribute('data-testid') ?? String(viewId(el)) : capability;
  Holds.push({ ticket, capability, name, el, args, answer });
  inflight.n++;
  say(`device ${capability} ${ticket} held (agent)`);
}
export const holds = () => {
  for (let i = Holds.length - 1; i >= 0; i--) if (Holds[i].el && !Holds[i].el.isConnected) { const [h] = Holds.splice(i, 1); inflight.n--; say(`device ${h.capability} ${h.ticket} retired`); }
  return Holds.map(h => ({ name: h.name, ticket: h.ticket, device: { capability: h.capability, args: h.args } }));
};
/** The agent's answer (D4): checked before the hold is spent. */
export async function answer(req) {
  holds();
  const at = Holds.findIndex(h => h.ticket === req.ticket);
  if (at < 0) return null;
  const h = Holds[at], value = req.op === 'type' ? String(req.text ?? '') : null;
  if (value == null && req.choice !== 'cancel' && !(h.capability === 'share' && req.choice === 'shared'))
    return { error: `@${h.ticket} (${h.capability}): tap takes cancel${h.capability === 'share' ? ' or shared' : ''}, not ${req.choice}` };
  const why = value != null ? h.check?.(value) ?? (h.capability === 'share' ? 'answered by tap, not type' : null) : null;
  if (why) return { error: `@${h.ticket} (${h.capability}): ${why}` };
  Holds.splice(at, 1); inflight.n--;
  const n = value == null ? 0 : paths(value).length;
  say(`device ${h.capability} ${h.ticket} ${value == null ? (req.choice === 'cancel' ? 'cancelled' : `answered: ${req.choice}`) : h.capability === 'pick' ? `answered: ${n} ${n === 1 ? 'file' : 'files'}` : h.capability === 'export' ? 'answered: a file' : `answered: ${n} ${n === 1 ? 'item' : 'items'}`}`);
  const out = await h.answer(value, req);
  return { ticket: h.ticket, capability: h.capability, answered: value == null ? req.choice : 'value', delivery: 'substituted', ...out };
}
Object.assign(x(), { files: { holds, answer } });

// ---------------------------------------------------------------- showPicker (picker.rs)
const tokens = el => (el.getAttribute('accept') ?? '').split(',').map(t => t.trim().toLowerCase()).filter(Boolean);
const MIME = { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', gif: 'image/gif', webp: 'image/webp', heic: 'image/heic', heif: 'image/heif', avif: 'image/avif', tif: 'image/tiff', tiff: 'image/tiff',
  mp4: 'video/mp4', mov: 'video/quicktime', m4v: 'video/x-m4v', webm: 'video/webm', json: 'application/json', md: 'text/markdown', markdown: 'text/markdown', txt: 'text/plain' };
const accepts = (accept, name, mime) => accept.some(t => t === '.' + extension(name) || t === mime || (t.endsWith('/*') && mime.split('/')[0] === t.slice(0, -2)));
const paths = v => { const l = v.split('\n').map(s => s.trim()).filter(Boolean); return l.length === 1 ? l[0].split(/\s+/) : l; };
Hosts.showPicker = id => {
  const found = byId(String(id ?? ''));
  const el = found?.el.localName === 'input' && found.el.type === 'file' ? found.el : null;
  if (!el) return say(`picker: refused: no file input with id "${id}"`);
  if (!clock.agent) {
    try { el.showPicker(); } catch (e) { say(`picker: refused: ${e.name}`); counted(glue().then(m => m.cancel(viewId(el)))); }
    return;
  }
  if (Holds.some(h => h.capability === 'pick' && h.el === el)) return say(`picker: "${id}" is already open`);
  const accept = tokens(el), multiple = el.hasAttribute('multiple');
  hold('pick', el, { id: String(id), accept, multiple }, (value, req) => counted(glue().then(m => m.answer(viewId(el), value == null ? null : req.files ?? []))));
  const h = Holds.at(-1);
  // What the real picker would have let the person choose (`check_pick`), with iOS Safari's HEIC rule (D6).
  h.check = v => {
    if (!el.isConnected) return 'the file input is gone';
    const list = paths(v);
    if (!list.length) return 'pick takes one or more file paths; tap @t cancel dismisses it';
    if (list.length > 1 && !multiple) return `this input takes one file, not ${list.length}`;
    for (const p of list) {
      const name = p.split(/[\\/]/).pop(), mime = MIME[extension(name)] ?? 'application/octet-stream';
      const heic = mime === 'image/heic' || mime === 'image/heif', keeps = accept.some(t => ['image/*', 'image/heic', 'image/heif', '.heic', '.heif'].includes(t));
      const delivered = heic && !keeps && accept.some(t => t.startsWith('image/')) ? 'image/jpeg' : mime;
      if (!(delivered === mime && accepts(accept, name, mime)) && !accepts(accept, name, delivered)) return `${name} (${mime}) is not among accept="${accept.join(',')}"`;
    }
    return null;
  };
};

// ---------------------------------------------------------------- saveFile (save_file.rs)
Hosts.saveFile = (...args) => {
  const refuse = (why, el) => { say(`saveFile: refused: ${why}`); if (el) el.dispatchEvent(new Event('cancel')); };
  if (args.length !== 3 || args.some(a => typeof a !== 'string')) return refuse('saveFile takes (id, from, suggestedName), three strings');
  const [id, from, suggestedName] = args, found = byId(id);
  if (!found) return refuse(`no element with id "${id}"`);
  const rest = from.startsWith('app:/') ? from.slice(5).split('/') : [];
  if (rest.length < 2 || rest.some(p => !p || p === '.' || p === '..' || p.includes('\0'))) return refuse(`${from} is not an app:/ file`, found.el);
  if (!coversPath(appGrantSet(), 'fs.read', from)) return refuse(`${from} is outside the app's fs.read grants`, found.el);
  if (!suggestedName || suggestedName.length > 255 || suggestedName === '.' || suggestedName === '..' || /[/\\\x00-\x1f\x7f]/.test(suggestedName)) return refuse('suggestedName is not a file name', found.el);
  const r = { present: true, view: found.view, from, suggestedName };
  if (clock.agent) {
    hold('export', found.el, { id, from, suggestedName }, (value, req) => counted(glue().then(m => m.answerSave({ node: found.view, answered: value == null ? 'cancel' : 'value', request: r }, req.text))));
    Holds.at(-1).check = v => { v = v.trim(); return (v.startsWith('/') || /^.:\\/.test(v)) && !/[\\/]$/.test(v) ? null : 'type an absolute file path to save to'; };
    return;
  }
  // The save picker starts inside the press's activation, else a download.
  const chosen = typeof showSaveFilePicker === 'function' ? showSaveFilePicker({ suggestedName }) : null;
  chosen?.catch(() => {});
  counted(glue().then(m => m.save(r, chosen)));
};

// ---------------------------------------------------------------- share (share.rs)
Hosts.share = (title, text, url) => {
  const refuse = why => say(`share: refused: ${why}`);
  if (text == null && url == null) return refuse('needs text= or url=');
  if (url != null && !/^https?:\/\/[^/?#\s]\S*$/i.test(url)) return refuse('url is not an absolute http: or https: URL');
  // The pressed node anchors the sheet (the press's own event, in whose commit commands run).
  const source = globalThis.event?.currentTarget, anchor = source instanceof Element ? viewId(source) : null;
  if (clock.agent) return hold('share', null, { title: title ?? null, text: text ?? null, url: url ?? null, anchor }, (_, req) => { say(req.choice === 'shared' ? 'share: shared' : 'share: dismissed'); });
  if (typeof navigator.share !== 'function') return refuse('unavailable');
  navigator.share(Object.fromEntries(Object.entries({ title, text, url }).filter(([, v]) => v != null)))
    .then(() => 'share: shared', e => e?.name === 'AbortError' ? 'share: dismissed' : `share: refused: ${e?.name ?? e}`).then(say);
};

// ---------------------------------------------------------------- the document pickers (file_pickers.rs)
// What the person chooses becomes a `doc:/<n>/<name>` path the app reaches
// through `storage.fs` (the glue's handles; storage-request.js routes it).
let Docs = null;
const docs = () => Docs ??= import(new URL('./documents-glue.js', import.meta.url).href).then(() => x().documents.install({
  log: say, openFile: () => false,
  dispatch(view, kind, payload) {
    const el = Views.get(view);
    if (!el) return;
    if (kind === 27) el.dispatchEvent(new Event('cancel'));
    else el.dispatchEvent(new CustomEvent('change', { detail: payload }));
  },
}));
// Files dropped from outside (DOM's `drop`, studio diary R19): Chromium's
// handles, asked for while the event lasts (`getAsFileSystemHandle`), else
// each File read-only; of the types `file_handlers` declares, minted as a
// picker's choice is, and heard with the `DragEvent` record. A `dragover`
// that carries files is accepted, or the browser would open them itself.
OnHooks.drop = (e, f) => {
  e.addEventListener('dragover', ev => { if (ev.dataTransfer?.types?.includes('Files') && !e.closest('[inert]')) { ev.preventDefault(); ev.dataTransfer.dropEffect = 'copy'; } });
  e.addEventListener('drop', ev => {
    const dt = ev.dataTransfer;
    if (!dt?.files?.length || e.closest('[inert]')) return;
    ev.preventDefault(); ev.stopPropagation();
    const [offsetX, offsetY] = record(e, ev), held = [ev.shiftKey, ev.ctrlKey, ev.altKey, ev.metaKey];
    const handles = [...dt.items].filter(i => i.kind === 'file').map(i => i.getAsFileSystemHandle?.().catch(() => null) ?? null), files = [...dt.files];
    counted(docs().then(async m => {
      const found = await m.dropped(await Promise.all(handles), files);
      if (!found.length) return say('drop: refused: no file of a type this app declares');
      f([offsetX, offsetY, found, ...held]);
    }));
  });
};
const PICKERS = { showOpenFilePicker: ['open-file', 'showOpenFilePicker takes (id) or (id, multiple)'], showDirectoryPicker: ['open-directory', 'showDirectoryPicker takes (id)'], showSaveFilePicker: ['save-file', 'showSaveFilePicker takes (id, suggestedName)'] };
for (const [name, [capability, usage]] of Object.entries(PICKERS)) Hosts[name] = (...args) => {
  const refuse = (why, el) => { say(`${name}: refused: ${why}`); if (el) el.dispatchEvent(new Event('cancel')); };
  const [id, second] = args, rest = args.slice(1);
  if (typeof id !== 'string') return refuse(usage);
  const ok = capability === 'open-file' ? rest.length === 0 || (rest.length === 1 && typeof second === 'boolean')
    : capability === 'open-directory' ? rest.length === 0 : rest.length === 1 && typeof second === 'string';
  if (!ok) return refuse(usage);
  const multiple = capability === 'open-file' && second === true, suggestedName = capability === 'save-file' ? second : '';
  if (capability === 'save-file' && (!suggestedName || suggestedName.length > 255 || suggestedName === '.' || suggestedName === '..' || /[/\\\x00-\x1f\x7f]/.test(suggestedName))) return refuse('suggestedName is not a file name');
  const found = byId(id);
  if (!found) return refuse(`no element with id "${id}"`);
  const r = { present: true, view: found.view, id, multiple, suggestedName };
  if (clock.agent) {
    hold(capability, found.el, capability === 'open-file' ? { id, multiple } : capability === 'save-file' ? { id, suggestedName } : { id },
      (value, req) => counted(docs().then(m => m.answer({ node: found.view, answered: value == null ? 'cancel' : 'value' }, name, req.files))));
    Holds.at(-1).check = v => { const list = paths(v); return !list.length || list.some(p => !p.startsWith('/')) ? 'type an absolute path' : list.length > 1 && !multiple ? `one path, not ${list.length}` : null; };
    return;
  }
  // A save without the browser's picker downloads (documents-glue.js), as `saveFile` does.
  if (typeof globalThis[name] !== 'function' && capability !== 'save-file') return refuse('unavailable', found.el);
  // The browser's picker is called in the glue, after it loads: a picker
  // needs the activation, which a page that has loaded it already keeps.
  counted(docs().then(m => m.show(r, name)));
};
