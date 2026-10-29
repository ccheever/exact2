// LLP 1126.000 WP6: an entry address on the existing agent channel.
import { resolve } from 'node:path';
import { MalformedOperation } from './agent-keys.mjs';

const VALUES = new Set(['--app', '--storage', '--sim', '--session', '--open', '--world', '--plan', '--backend', '--browser', '--size', '--scale', '--test', '--url', '--seed', '--locale', '--format-locale', '--time-zone', '--epoch', '--timing', '--touch', '--chrome', '--phone', '--port', '--fail-fetch', '--family', '--mode', '--scheme', '--props', '--placement', '--state', '--permission', '--bind', '--notification', '--wake', '--call']);
const supported = 'widget: tree, state, "screenshot <png>" or "clock <ms|+ms|settle|data>"; screenshot film, window and compare options are unavailable';

export function widgetArguments(input) {
  const argv = [...input], positional = [];
  for (let i = 0; i < argv.length; i++) {
    if (VALUES.has(argv[i])) i++;
    else if (!argv[i].startsWith('--')) positional.push(i);
  }
  const kind = argv[positional[1]];
  if (!['widget', 'activity'].includes(kind)) return { argv };
  if (!['ios', 'macos', 'mac'].includes(argv[positional[0]])) throw new Error(`${kind}: use ios or macos`);
  const at = positional[1], entry = argv[at + 1];
  if (!entry || entry.startsWith('--')) throw new Error(`${kind} <entry>: name an entry`);
  const widget = { entry };
  argv.splice(at, 2);
  for (let i = 0; i < argv.length;) {
    const local = kind === 'activity' ? ['--placement', '--scheme', '--state', '--scale'].includes(argv[i]) : ['--family', '--mode', '--scheme', '--size', '--props', '--scale'].includes(argv[i]);
    if (local) {
      const flag = argv[i], value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error(`${flag} needs a value`);
      widget[flag.slice(2)] = value;
      argv.splice(i, 2);
    } else if (VALUES.has(argv[i])) i += 2;
    else i++;
  }
  const jsonFlag = kind === 'activity' ? 'state' : 'props';
  if (widget[jsonFlag] !== undefined) {
    let props;
    try { props = JSON.parse(widget[jsonFlag]); } catch { throw new Error(`${kind} --${jsonFlag}: expected a JSON object`); }
    if (!props || Array.isArray(props) || typeof props !== 'object') throw new Error(`${kind} --${jsonFlag}: expected a JSON object`);
  }
  if (widget.mode === 'full-color') widget.mode = 'fullColor';
  // The stand-in renders one pixel per point unless asked: a device's 2 or 3 shows the picture as the widget draws it.
  if (widget.scale !== undefined && !['1', '2', '3'].includes(widget.scale)) throw new Error(`${kind} --scale: 1, 2 or 3`);
  if (kind === 'activity') {
    widget.placement ??= 'lockScreen';
    if (!['lockScreen', 'compactLeading', 'compactTrailing', 'minimal', 'expanded'].includes(widget.placement)) throw new Error('activity --placement: use lockScreen, compactLeading, compactTrailing, minimal or expanded');
  }
  return { argv, [kind]: widget };
}

export function widgetOperation(line, kind = 'widget') {
  const text = line.trim();
  if (text === 'tree' || text === 'state') return { op: text };
  const image = /^screenshot\s+(\S+\.png)$/.exec(text);
  if (image) return { op: 'screenshot', path: resolve(image[1]) };
  const clock = /^clock(?:\s+(.+))?$/.exec(text);
  if (clock) return { op: 'clock', value: clock[1] ?? 'settle' };
  throw new MalformedOperation(supported.replace('widget:', `${kind}:`));
}

export function validateWidget(flags, rest) {
  const kind = flags.activity ? 'activity' : 'widget';
  if (flags.test || flags.plan || flags.url || flags.device) throw new Error(`${kind}: use a development app on macos or an iOS simulator`);
  for (const line of rest.slice(1)) widgetOperation(line, kind);
}

function ordered(value) {
  if (Array.isArray(value)) return value.map(ordered);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, ordered(value[key])]));
  return value;
}

const availability = new WeakMap();
export async function widgetStep(session, widget, line, kind = 'widget') {
  const address = { [kind]: widget };
  const { op, path, value } = widgetOperation(line, kind);
  if (op === 'clock') {
    if (!availability.has(session)) availability.set(session, new Map());
    const addresses = availability.get(session), key = JSON.stringify(ordered(address));
    if (!addresses.has(key)) addresses.set(key, session.op({ op: 'state', ...address, availability: true }));
    const available = await addresses.get(key);
    if (available.error) throw new MalformedOperation(available.error);
  }
  return [op, op === 'clock' ? await session.clock(value) : ordered(await session.op({ op, ...address, ...(path ? { path } : {}) }))];
}
