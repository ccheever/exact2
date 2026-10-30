import { MalformedOperation, pointerArguments, pinchArguments, intoArguments, resizeArguments, tapWords, typeArguments } from '../../scripts/agent-keys.mjs';
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
    // A file drag from outside (`drop|dragenter <path…>`, `dragleave`), as the driver takes it.
    case 'tap': { const target = word(); if (['drop', 'dragenter', 'dragleave'].includes(words[0])) { try { call('tap', ...tapWords([target, ...words.splice(0)])); } catch (e) { invalid(e.message); } break; } if (words[0] !== 'at') { call('tap', target); break; } words.shift(); call('tap', target, { at: [Number(word()), Number(word())] }); break; } // a press at a point of the target, from its top left
    case 'menu': call('tap', word(), { contextmenu: true }); break;
    case 'type': { const target = word(); if (words[0] === 'paste' && ['--file', '--image'].includes(words[1])) { try { call('type', ...typeArguments([target, ...words.splice(0)])); } catch (e) { invalid(e.message); } break; } const text = [word(), ...words.splice(0)].join(' '); call('type', target, text); break; } // a paste of files or an image's data, as the driver takes it
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

/** A clock refusal's runner error, as each target words it: the wasm page's `clock: Poisoned`, the JS target's
 * `clock: t=0 refused advance: Poisoned` (its journal line). */
export const clockRefusal = message => message.split('\n')[0].replace(/^(clock: )+/, '').replace(/^t=\S+ refused advance: /, '');

/** Every target's reply to one clock operation, `[who, { value } | { error }]`, as a step. All answered: the
 * values. All refusing with the same `Poisoned`: that refusal, which the drive records as the step and goes past,
 * as a poisoned runner refuses every forward advance (semantics/Contract/Axiomatic.lean `advance_poisoned`).
 * Anything else: a failure naming what each did. */
export function clockVerdict(replies) {
  const refused = replies.filter(([, r]) => 'error' in r).map(([, r]) => clockRefusal(r.error));
  if (!refused.length) return { answered: replies.map(([, r]) => r.value) };
  if (refused.length === replies.length && refused.every(x => x === 'Poisoned')) return { refused: 'Poisoned' };
  return { failure: replies.map(([who, r]) => `${who} ${'error' in r ? `refused «${clockRefusal(r.error)}»` : 'answered'}`).join(', ') };
}

/** One clock operation `arg` on the pages (`[[who, session], …]`) and, when the pages refuse, on Linux too (a
 * session, or null where it does not run): `{ answered }` (Linux then runs as any step does), `{ refused, compared }`
 * when every target refused alike with `Poisoned`, `{ refused, linux }` when the pages did and Linux did not (a
 * failure), or `{ failure }` when the pages disagree. A malformed reply throws. */
export async function clockOnAll(arg, pages, linux, pair) {
  const reply = run => run().then(value => ({ value }), e => { if (e.malformed === true) throw e; return { error: e.message }; });
  const replies = await pair(...pages.map(([, s]) => () => reply(() => s.clock(arg))));
  const named = pages.map(([who], i) => [who, replies[i]]);
  const verdict = clockVerdict(named);
  if (!verdict.refused || !linux) return { ...verdict, compared: pages.map(([who]) => who) };
  const all = clockVerdict([...named, ['linux', await reply(() => linux.clock(arg))]]);
  return all.refused ? { ...all, compared: [...pages.map(([who]) => who), 'linux'] } : { refused: verdict.refused, linux: all.failure };
}
