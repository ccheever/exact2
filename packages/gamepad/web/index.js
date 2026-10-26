// @exact/gamepad on the web (LLP 1133.022 D1, D5, D12): the browser's own
// registry, `navigator.getGamepads()`, in the interface's shapes. The page
// runtime (host/web-js/modules.js) calls it outside the agent.
//
// - The browser exposes no pad until a gamepad user gesture (a button
//   pressed or a stick moved after reading neutral), so the list reads
//   empty until then; that is the platform's.
// - The web has no event for a press: while a reader reads the list and the
//   page is visible, the pads are polled on animation frames to follow
//   `current` (the pad whose button last went down; a stick's drift is not
//   use), and announced only when what a reader's answer carries changes.
//   A reader with `interval` also reads each pad's buttons and axes, at most
//   one announcement per interval.
// - A send without `index` reads `getGamepads()` once more at receipt, so a
//   press in the same frame counts.
// - Effects are `vibrationActuator`'s where the engine has one (Chromium,
//   Safari on macOS; not Firefox or Safari on iOS); no pad has a light here.

/** D2's labels by family; the interface's `tables.labels` (a test checks they agree). */
export const LABELS = {
  xbox: ['A', 'B', 'X', 'Y', 'LB', 'RB', 'LT', 'RT', 'View', 'Menu', 'LS', 'RS', 'Up', 'Down', 'Left', 'Right', 'Xbox'],
  playstation: ['Cross', 'Circle', 'Square', 'Triangle', 'L1', 'R1', 'L2', 'R2', 'Create', 'Options', 'L3', 'R3', 'Up', 'Down', 'Left', 'Right', 'PS'],
  nintendo: ['B', 'A', 'Y', 'X', 'L', 'R', 'ZL', 'ZR', '−', '+', 'LS', 'RS', 'Up', 'Down', 'Left', 'Right', 'Home'],
  generic: ['A', 'B', 'X', 'Y', 'L1', 'R1', 'L2', 'R2', 'Select', 'Start', 'L3', 'R3', 'Up', 'Down', 'Left', 'Right', 'Home'],
};

const VENDORS = { '045e': 'xbox', '054c': 'playstation', '057e': 'nintendo' };

/** The face buttons' scheme from the engine's id: Chromium's `Vendor: 054c`, Firefox's `054c-0ce6-…`, else the name. */
export function family(id) {
  const vendor = /Vendor:\s*([0-9a-f]{4})/i.exec(id)?.[1] ?? /^([0-9a-f]{4})-[0-9a-f]{4}-/i.exec(id)?.[1];
  if (vendor && VENDORS[vendor.toLowerCase()]) return VENDORS[vendor.toLowerCase()];
  if (/dualsense|dualshock|playstation|wireless controller/i.test(id)) return 'playstation';
  if (/xbox|xinput/i.test(id)) return 'xbox';
  if (/nintendo|pro controller|joy-con/i.test(id)) return 'nintendo';
  return 'generic';
}

/** What `playEffect` can play on `pad`. */
function effects(pad) {
  const actuator = pad.vibrationActuator;
  if (!actuator || typeof actuator.playEffect !== 'function') return [];
  const listed = Array.isArray(actuator.effects) ? actuator.effects : [actuator.type ?? 'dual-rumble'];
  return listed.filter(e => e === 'dual-rumble' || e === 'trigger-rumble');
}

const DOWN = 0.5;
const STATUSES = { complete: 'complete', preempted: 'preempted' };

export function implementation({ changed, emit, foreground }) {
  const has = typeof navigator === 'object' && typeof navigator.getGamepads === 'function';
  const read = () => (has ? [...navigator.getGamepads()].filter(p => p && p.connected) : []);
  // Each pad's buttons last seen down, by index, and the pad last used.
  const down = new Map();
  let current = null, said = null, lastValues = 0, frame = null;
  // The intervals the readers asked for; a reader without one reads identity only.
  const intervals = new Map();

  /** `current` follows the pad whose button last went down. */
  function follow(pads) {
    for (const pad of pads) {
      const now = pad.buttons.map(b => b.pressed || b.value > DOWN);
      const before = down.get(pad.index) ?? [];
      if (now.some((d, i) => d && !before[i])) current = pad.index;
      down.set(pad.index, now);
    }
    for (const index of down.keys()) if (!pads.some(p => p.index === index)) down.delete(index);
    if (current != null && !pads.some(p => p.index === current)) current = null;
  }

  function shape(pad, values) {
    const f = family(pad.id);
    const out = {
      index: pad.index, id: pad.id, mapping: pad.mapping ?? '', family: f,
      labels: pad.mapping === 'standard' ? LABELS[f] : [], effects: effects(pad), light: false,
      buttons: null, axes: null, timestamp: null, windows: null, linux: null,
    };
    if (values && pad.mapping === 'standard') {
      out.buttons = pad.buttons.map(b => ({ pressed: b.pressed, touched: b.touched ?? b.pressed, value: b.value }));
      out.axes = [...pad.axes];
      out.timestamp = pad.timestamp;
    }
    return out;
  }

  const identity = pads => JSON.stringify([current, pads.map(p => [p.index, p.id, p.mapping, effects(p)])]);
  const values = pads => JSON.stringify(pads.map(p => [p.buttons.map(b => b.value), [...p.axes]]));
  let lastSeen = '';

  function tick() {
    frame = null;
    if (intervals.size === 0 || !foreground()) return;
    const pads = read();
    follow(pads);
    const id = identity(pads);
    const min = Math.min(...[...intervals.values()].filter(v => v != null), Infinity);
    const seen = Number.isFinite(min) ? values(pads) : '';
    const at = performance.now();
    if (id !== said) {
      said = id;
      lastSeen = seen;
      lastValues = at;
      changed('gamepad.getGamepads');
    } else if (seen !== lastSeen && at - lastValues >= min) {
      lastSeen = seen;
      lastValues = at;
      changed('gamepad.getGamepads');
    }
    frame = requestAnimationFrame(tick);
  }
  const poll = () => { if (frame == null && intervals.size > 0 && typeof requestAnimationFrame === 'function') frame = requestAnimationFrame(tick); };
  const stop = () => { if (frame != null) cancelAnimationFrame(frame); frame = null; };

  if (typeof addEventListener === 'function') {
    addEventListener('gamepadconnected', e => { follow(read()); emit('gamepadconnected', [shape(e.gamepad, false)]); changed('gamepad.getGamepads'); });
    addEventListener('gamepaddisconnected', e => { follow(read()); emit('gamepaddisconnected', [shape(e.gamepad, false)]); changed('gamepad.getGamepads'); });
  }

  /** The pad a send names: its `index`, else the current one at receipt. */
  function named(args) {
    const pads = read();
    follow(pads);
    const index = args?.index ?? current;
    return index == null ? null : pads.find(p => p.index === index) ?? null;
  }

  return {
    getGamepads(args) {
      intervals.set(JSON.stringify(args ?? {}), args?.interval ?? null);
      const pads = read();
      follow(pads);
      said = identity(pads);
      poll();
      return { available: has, gamepads: pads.map(p => shape(p, args?.interval != null)), current };
    },
    async playEffect(args) {
      const pad = named(args);
      if (!pad) return { error: 'NotFoundError' };
      if (!effects(pad).includes(args.type)) return { error: 'NotSupportedError' };
      const params = {};
      for (const k of ['duration', 'startDelay', 'strongMagnitude', 'weakMagnitude', 'leftTrigger', 'rightTrigger']) if (args[k] != null) params[k] = args[k];
      try {
        const result = await pad.vibrationActuator.playEffect(args.type, params);
        return { result: STATUSES[result] ?? 'complete' };
      } catch (e) {
        return { error: e?.name === 'TypeError' ? 'TypeError' : 'InvalidStateError' };
      }
    },
    async resetEffects(args) {
      const pad = named(args);
      if (!pad) return { error: 'NotFoundError' };
      const actuator = pad.vibrationActuator;
      if (!actuator || typeof actuator.reset !== 'function') return { result: 'complete' };
      try {
        return { result: STATUSES[await actuator.reset()] ?? 'complete' };
      } catch {
        return { error: 'InvalidStateError' };
      }
    },
    async setLight(args) {
      if (args?.color == null && args?.index == null) return {};
      const pad = named(args);
      return pad ? { error: 'NotSupportedError' } : { error: 'NotFoundError' };
    },
    foreground(shown) {
      if (shown) poll();
    },
    /** No reader asks with these arguments any longer. */
    releaseArgs(source, args) {
      if (source !== 'getGamepads') return;
      intervals.delete(JSON.stringify(args ?? {}));
      if (intervals.size === 0) stop();
    },
    /** The last reader of `source` is gone. */
    release(source) {
      if (source !== 'getGamepads') return;
      intervals.clear();
      stop();
    },
  };
}
