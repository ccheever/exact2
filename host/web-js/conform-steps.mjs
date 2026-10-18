import { MalformedOperation, pointerArguments, pinchArguments, intoArguments, resizeArguments } from '../../scripts/agent-keys.mjs';
import { dragArguments } from '../../scripts/agent-drag.mjs';

/** Consume the harness's words; reject malformed input before a browser can skip it. */
export function parseStep(line) {
  const words = line.trim().split(/\s+/), mustRefuse = words[0] === 'refused';
  if (mustRefuse) words.shift();
  const op = words.shift(), calls = [];
  const invalid = message => { throw new MalformedOperation(`not a step: ${line}: ${message}`); };
  const word = () => words.length ? words.shift() : invalid(`missing argument for ${op}`);
  const optional = fallback => words.length ? word() : fallback;
  const check = validate => { try { validate(); } catch (e) { invalid(e.message); } };
  const call = (method, ...args) => {
    if (method === 'pointer') check(() => pointerArguments(args[0], args[1] ?? {}));
    if (method === 'tap' && args[1]?.drag) check(() => dragArguments(args[1].drag));
    if (method === 'tap' && args[1]?.pinch !== undefined) check(() => pinchArguments(args[1].pinch));
    if (method === 'tap' && args[1]?.into) check(() => intoArguments(args[1].into));
    calls.push([method, args]);
  };
  switch (op) {
    case 'resize': {
      const size = word();
      try { call('resize', ...resizeArguments(size)); } catch (e) { invalid(e.message); }
      break;
    }
    case 'tap': call('tap', word()); break;
    case 'menu': call('tap', word(), { contextmenu: true }); break;
    case 'type': { const target = word(), text = [word(), ...words.splice(0)].join(' '); call('type', target, text); break; }
    case 'key': {
      const target = word(), key = word(), phase = optional(undefined);
      call('type', target, { key, ...(phase === 'for' ? { for: Number(word()) } : phase ? { phase } : {}) });
      break;
    }
    case 'clock': call('clock', word()); break;
    case 'back': call('tap', word(), { history: -1 }); break;
    case 'wheel': { const target = word(), dy = Number(word()), dx = Number(optional(0)); call('tap', target, { wheel: [dx, dy] }); break; }
    case 'into': { const target = word(), key = word(), block = optional(undefined); call('tap', target, { into: { key, ...(block ? { block } : {}) } }); break; }
    case 'pinch': call('tap', word(), { pinch: Number(word()) }); break;
    case 'down': call('tap', word(), { down: true }); break;
    case 'up': call('pointer', 'up'); break;
    case 'move': call('pointer', 'move', { dx: Number(word()), dy: Number(word()), ms: Number(optional(200)) }); break;
    case 'drag': {
      const target = word(), x = word();
      if (x === 'to') { call('tap', target, { drag: { to: word(), over: Number(optional(200)) } }); break; }
      const dx = Number(x), dy = Number(word()), over = Number(optional(200));
      if (words.length) {
        if (word() !== 'hold') invalid('drag takes hold <ms> after its duration');
        call('tap', target, { drag: { dx, dy, over, hold: Number(word()) } });
      } else {
        const move = { dx, dy, ms: over };
        call('tap', target, { down: true }); call('pointer', 'move', move); call('pointer', 'up');
      }
      break;
    }
    case 'mediasession': {
      const target = word(), mediaSession = word(), seconds = optional(undefined);
      call('tap', target, { mediaSession, ...(seconds != null ? { seconds: Number(seconds) } : {}) });
      break;
    }
    // A MIDI input's message (LLP 1125 §4): `midi <input> <hex…>`; its clock (D9): `midi <input> clock <bpm> <ms>`.
    case 'midi': {
      const input = word();
      if (words[0] === 'clock') { words.shift(); call('tap', null, { midi: { input, clock: { bpm: Number(word()), ms: Number(word()) } } }); break; }
      const data = words.splice(0).map(w => /^[0-9a-f]{2}$/i.test(w) ? parseInt(w, 16) : invalid(`${w} is not a byte (two hex digits)`));
      if (!data.length) invalid('midi <input> <hex…>');
      call('tap', null, { midi: { input, data } }); break;
    }
    case 'prefer': {
      const facts = [[word(), word()]];
      while (words.length) facts.push([word(), word()]);
      call('prefer', Object.fromEntries(facts)); break;
    }
    default: invalid(op ? `unknown operation ${op}` : 'missing operation');
  }
  if (words.length) invalid(`unconsumed words: ${words.join(' ')}`);
  return { op, mustRefuse, async run(session) {
    try {
      for (const [method, args] of calls) MalformedOperation.reply(await session[method](...args));
    } catch (e) {
      if (e.malformed === true) invalid(e.message);
      throw e;
    }
  } };
}
