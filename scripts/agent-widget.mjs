// LLP 1126.000 WP6: an entry address on the existing agent channel.
import { resolve } from 'node:path';
import { MalformedOperation } from './agent-keys.mjs';

const VALUES = new Set(['--app', '--storage', '--sim', '--session', '--open', '--world', '--plan', '--backend', '--browser', '--size', '--scale', '--test', '--url', '--seed', '--locale', '--time-zone', '--epoch', '--timing', '--touch', '--chrome', '--phone', '--port', '--fail-fetch', '--family', '--mode', '--scheme', '--props']);
const supported = 'widget: tree, state, "screenshot <png>" or "clock <ms|+ms|settle|data>"; screenshot film, window and compare options are unavailable';

export function widgetArguments(input) {
  const argv = [...input], positional = [];
  for (let i = 0; i < argv.length; i++) {
    if (VALUES.has(argv[i])) i++;
    else if (!argv[i].startsWith('--')) positional.push(i);
  }
  if (argv[positional[1]] !== 'widget') return { argv };
  if (!['ios', 'macos', 'mac'].includes(argv[positional[0]])) throw new Error('widget: use ios or macos');
  const at = positional[1], entry = argv[at + 1];
  if (!entry || entry.startsWith('--')) throw new Error('widget <entry>: name an entry');
  const widget = { entry };
  argv.splice(at, 2);
  for (let i = 0; i < argv.length;) {
    if (['--family', '--mode', '--scheme', '--size', '--props'].includes(argv[i])) {
      const flag = argv[i], value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error(`${flag} needs a value`);
      widget[flag.slice(2)] = value;
      argv.splice(i, 2);
    } else if (VALUES.has(argv[i])) i += 2;
    else i++;
  }
  if (widget.props !== undefined) {
    let props;
    try { props = JSON.parse(widget.props); } catch { throw new Error('widget --props: expected a JSON object'); }
    if (!props || Array.isArray(props) || typeof props !== 'object') throw new Error('widget --props: expected a JSON object');
  }
  if (widget.mode === 'full-color') widget.mode = 'fullColor';
  return { argv, widget };
}

export function widgetOperation(line) {
  const text = line.trim();
  if (text === 'tree' || text === 'state') return { op: text };
  const image = /^screenshot\s+(\S+\.png)$/.exec(text);
  if (image) return { op: 'screenshot', path: resolve(image[1]) };
  const clock = /^clock(?:\s+(.+))?$/.exec(text);
  if (clock) return { op: 'clock', value: clock[1] ?? 'settle' };
  throw new MalformedOperation(supported);
}

export function validateWidget(flags, rest) {
  if (flags.test || flags.plan || flags.url || flags.device) throw new Error('widget: use a development app on macos or an iOS simulator');
  if (flags.scale !== undefined) throw new Error('widget: --scale is unavailable; snapshots use one pixel per point');
  for (const line of rest.slice(1)) widgetOperation(line);
}

function ordered(value) {
  if (Array.isArray(value)) return value.map(ordered);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, ordered(value[key])]));
  return value;
}

const availability = new WeakMap();
export async function widgetStep(session, widget, line) {
  const { op, path, value } = widgetOperation(line);
  if (op === 'clock') {
    if (!availability.has(session)) availability.set(session, session.op({ op: 'state', widget, availability: true }));
    const available = await availability.get(session);
    if (available.error) throw new MalformedOperation(available.error);
  }
  return [op, op === 'clock' ? await session.clock(value) : ordered(await session.op({ op, widget, ...(path ? { path } : {}) }))];
}
