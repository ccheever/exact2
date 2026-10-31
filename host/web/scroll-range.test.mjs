import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { layoutBoxes, agentInput } from './agent-input.js';

// The wasm host loads the agent's helpers as `agentInput` and binds each by name (glue.js `agentInputReady`): a name
// it binds that `agentInput` lacks is undefined at the first call, as `scrollRange` once was in every `layout <target>`.
test('the wasm host binds no agent helper that agent-input.js does not hand it', () => {
  const glue = readFileSync(new URL('./glue.js', import.meta.url), 'utf8');
  const bound = /\(\{ ([\w, ]+) \} = helpers\)/.exec(glue)?.[1].split(', ');
  expect(bound).toContain('scrollRange');
  for (const name of bound) expect([name, typeof agentInput[name]]).toEqual([name, 'function']);
});

test('the wasm host\'s layout reports a scroller\'s range beside its offset, a hidden overflow\'s too (LLP 1012 rx/ry)', () => {
  const saved = { style: globalThis.getComputedStyle, frame: globalThis.HTMLIFrameElement };
  globalThis.getComputedStyle = () => ({ display: 'block' });
  globalThis.HTMLIFrameElement ??= class {};
  try {
    const element = (dataset, size) => ({ isConnected: true, dataset, scrollLeft: 0, scrollTop: 40, ...size, hasAttribute: () => false, getBoundingClientRect: () => ({ x: 0, y: 0, width: 400, height: 600 }) });
    const [plain, wide, fits] = layoutBoxes(new Map([
      [1, element({}, {})],
      // `overflow-x: hidden` over content 84 wider: a programmatic range all the same
      [2, element({ scroll: 'true' }, { scrollWidth: 484, clientWidth: 400, scrollHeight: 1200, clientHeight: 600 })],
      [3, element({ scroll: 'true' }, { scrollWidth: 300, clientWidth: 400, scrollHeight: 600, clientHeight: 600 })],
    ]));
    expect(plain.rx).toBeUndefined();
    expect(wide).toMatchObject({ sx: 0, sy: 40, rx: 84, ry: 600 });
    expect(fits).toMatchObject({ rx: 0, ry: 0 });
  } finally {
    globalThis.getComputedStyle = saved.style;
    globalThis.HTMLIFrameElement = saved.frame;
  }
});
