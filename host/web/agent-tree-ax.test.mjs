// `tree --ax` over hand-written replies: the findings, parity and transcript (moved whole from agent.test.mjs, its
// 1,500-line cap).
import { test, expect } from 'bun:test';
// @ref LLP 1080.002 D7–D9 — the findings, parity and transcript over hand-written replies.
import { axFindings, axParity, axClean, axFinish, renderAx, webAx, axTree } from '../../scripts/agent-ax.mjs';
const axReply = (elements, extra = {}) => ({ epoch: 3, incarnation: 1, clock: 0, ax: { source: 'chrome-cdp', platform: 'Chrome 1', order: 'tree',
  coverage: { roots: ['document'], complete: true, visited: elements.length }, modal: { present: false }, elements, ...extra } });
const el = (i, role, name, more = {}) => ({ i, parent: null, id: i + 10, via: 'self', role, name, states: {}, interactive: ['button', 'link', 'textbox', 'checkbox'].includes(role), native: { role }, ...more });
const axPlain = { nodes: [
  { id: 10, parent: null, props: { testId: 'play' }, children: [] },
  { id: 11, parent: null, props: { testId: 'box', inert: true }, children: [12] },
  { id: 12, parent: 11, props: { testId: 'inside' }, children: [] },
  { id: 13, parent: null, props: { testId: 'sheet' }, children: [14] },
  { id: 14, parent: 13, props: { testId: 'ok' }, children: [] },
  { id: 15, parent: null, props: { testId: 'behind' }, children: [] } ] };

test('tree --ax: unnamed fires on an interactive element with no name and is silent once named', () => {
  const unnamed = axFinish(axReply([el(0, 'button', '')]), axPlain, null).ax.findings;
  expect(unnamed.map(f => [f.kind, f.testId])).toEqual([['unnamed', 'play']]);
  expect(axFinish(axReply([el(0, 'button', 'Play')]), axPlain, null).ax.findings).toEqual([]);
  // Text is not interactive: an empty name there is no finding.
  expect(axFinish(axReply([el(0, 'StaticText', '')]), axPlain, null).ax.findings).toEqual([]);
});

test('tree --ax: exposed-hidden fires under an inert ancestor in intent and is silent outside it', () => {
  const r = axFinish(axReply([el(2, 'button', 'Hidden')]), axPlain, null);
  expect(r.ax.findings.map(f => [f.kind, f.testId, f.under])).toEqual([['exposed-hidden', 'inside', 11]]);
  expect(r.ax.intent[11]).toEqual({ inert: true, testId: 'box' });
  expect(axFinish(axReply([el(4, 'button', 'Ok')]), axPlain, null).ax.findings).toEqual([]);
});

test('tree --ax: outside-modal fires for an element outside the open modal, by the platform or UIKit\'s rule', () => {
  const sheet = el(3, 'dialog', 'Sheet'), ok = el(4, 'button', 'Ok', { parent: 0 }), behind = el(5, 'button', 'Behind');
  sheet.i = 0; ok.i = 1; behind.i = 2;
  const modal = { present: true, element: 0, id: 13, by: 'dialog:modal' };
  const web = axFinish(axReply([sheet, ok, behind], { modal }), axPlain, null).ax.findings;
  expect(web.map(f => [f.kind, f.testId, f.basis])).toEqual([['outside-modal', 'behind', 'platform']]);
  const kit = axFinish(axReply([sheet, ok, { ...behind, outsideModal: false }], { modal, source: 'uikit' }), axPlain, null).ax.findings;
  expect(kit).toEqual([]);
  const leak = axFinish(axReply([sheet, ok, { ...behind, outsideModal: true }], { modal, source: 'uikit' }), axPlain, null).ax.findings;
  expect(leak.map(f => [f.kind, f.basis])).toEqual([['outside-modal', 'documented-rule']]);
});

test('tree --ax: a targeted read keeps its subtree, the chain above it, and its ancestors\' intent', () => {
  const box = el(1, 'group', 'Box'), inside = el(2, 'button', 'Hidden', { parent: 0 });
  box.i = 0; inside.i = 1;
  const r = axFinish(axReply([el(0, 'button', 'Play'), box, inside].map((e, i) => ({ ...e, i, parent: e === inside ? 1 : null }))), axPlain, 12);
  expect(r.ax.elements.map(e => e.testId)).toEqual(['inside']);
  expect(r.ax.ancestors).toEqual([{ id: 11, role: 'group', name: 'Box' }]);
  expect(r.ax.intent[11].inert).toBe(true);
});

test('tree --ax: parity joins by unique testId, normalizes the fixture controls, and refuses a partial side', () => {
  const web = axReply([el(0, 'checkbox', 'Checked', { testId: 'check', states: { checked: true } }), el(1, 'button', 'Add', { testId: 'add' }), el(2, 'button', 'A', { testId: 'dup' }), el(3, 'button', 'B', { testId: 'dup' })]);
  const ios = axReply([ // UIKit's form of a checkbox: a toggle whose value is "1" (LLP 1115.000 D13.3; agent-ax.test.mjs).
    el(0, 'checkbox', 'Checked', { testId: 'check', value: '1', native: { role: ['button', 'toggleButton'] } }),
    el(1, 'button', 'Plus', { testId: 'add', native: { role: ['button'] } })], { source: 'uikit' });
  const { findings, unjoined } = axParity(web, ios);
  expect(findings.map(f => f.detail)).toEqual(['name "Add" (web) vs "Plus" (uikit)']);
  expect(unjoined).toEqual([]);
  expect(() => axParity({ ...web, spanned: true }, ios)).toThrow(/spanned/);
  // A drawn radio is a radio, its checked state the UIKit reply's own (x2apps survey #2).
  const radioWeb = axReply([el(0, 'radio', 'Red', { testId: 'red', states: { checked: true } })]);
  const radioIos = axReply([el(0, 'button', 'Red', { testId: 'red', states: { checked: true, selected: true }, native: { role: ['button', 'selected'], class: 'ExactRadio' } })], { source: 'uikit' });
  expect(axParity(radioWeb, radioIos).findings).toEqual([]);
});

test('tree --ax: no findings means clean only with complete coverage and the expected views joined', () => {
  const r = axFinish(axReply([el(0, 'button', 'Play')]), axPlain, null);
  expect(axClean(r, ['play'])).toBe(true);
  expect(axClean(r, ['missing'])).toBe(false);
  const partial = axReply([el(0, 'button', 'Play')], { truncated: { elements: 'unknown', fields: 0 } });
  expect(axClean(partial)).toBe(false);
  expect(renderAx(partial)).toContain('(no findings — coverage incomplete)');
});

test('tree --ax renders each element with its join and frame, then its findings', () => {
  const r = axFinish(axReply([el(0, 'button', '', { frame: { x: 1, y: 2, w: 3, h: 4, source: 'layout' } }), el(2, 'button', 'Hidden', { via: 'owner' }), el(5, 'textbox', 'Your name', { description: 'Required', states: { required: true } })]), axPlain, null);
  const text = renderAx(r);
  expect(text).toMatch(/^ax {7}chrome-cdp · Chrome 1 · order tree · epoch 3 · incarnation 1 · clock 0 ms · 3 elements/);
  expect(text).toContain('button "" #10 [play] 1,2 3×4');
  expect(text).toContain('button "Hidden" #12^ [inside]');
  // The accessible description (aria-describedby) is printed: the survey diary could not see it.
  expect(text).toContain('textbox "Your name" description="Required" [required] #15 [behind]');
  expect(text).toContain('! unnamed button #10 [play]');
  expect(text).toContain('! exposed while hidden: button "Hidden" under #11 (inert) #12 [inside]');
  expect(renderAx({ ax: { unavailable: true, reason: 'no AT-SPI tree (LLP 1015 §7)' } })).toBe('ax       unavailable: no AT-SPI tree (LLP 1015 §7)');
});

const fakeChrome = ({ nodes, stamps, plain }) => {
  const order = [];
  const snapshot = { strings: ['data-agent-view', '1'], documents: [{ nodes: { backendNodeId: [100], attributes: [[0, 1]], parentIndex: [-1], nodeType: [1], nodeName: [0] }, layout: { nodeIndex: [], bounds: [] } }] };
  return { order, carrier: { browser: 'chrome', axEnabled: 'Chrome 1', async call(m) { order.push(m); return m === 'Accessibility.getFullAXTree' ? { nodes } : snapshot; },
    async evaluate() { return 'Chrome/1'; }, async ask() { order.push('stamp'); return stamps.shift(); } },
    readPlain: async () => { order.push('plain'); return plain.shift(); } };
};
const button = { nodeId: 'b', ignored: false, role: { value: 'button' }, name: { value: 'Go' }, backendDOMNodeId: 100, childIds: [] };

test('tree --ax (web): the plain tree is read inside the document bracket, and a reload between retries the whole read', async () => {
  const stamp = (nonce, epoch = 1) => ({ nonce, epoch, incarnation: 1, clock: 0 });
  const f = fakeChrome({ nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['b'] }, button],
    stamps: [stamp(1), stamp(2), stamp(2), stamp(2)], plain: [{ epoch: 1, incarnation: 1, nodes: [] }, { epoch: 1, incarnation: 1, nodes: [{ id: 1, props: { testId: 'new' }, children: [] }] }] });
  const { reply, plain } = await webAx(f.carrier, {}, f.readPlain);
  expect(f.order.slice(0, 5)).toEqual(['stamp', 'plain', 'Accessibility.getFullAXTree', 'DOMSnapshot.captureSnapshot', 'stamp']);
  expect(reply.spanned).toBeUndefined();
  expect(plain.nodes[0].props.testId).toBe('new'); // the second document's intent, never the first's
});

test('tree --ax (web): a depth cutoff is incomplete coverage with an unknown remainder, never clean', async () => {
  const chain = Array.from({ length: 300 }, (_, k) => ({ nodeId: `g${k}`, ignored: true, role: { value: 'generic' }, childIds: [k < 299 ? `g${k + 1}` : 'b'] }));
  const f = fakeChrome({ nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['g0'] }, ...chain, button], stamps: [{ nonce: 1, epoch: 1, incarnation: 1 }, { nonce: 1, epoch: 1, incarnation: 1 }], plain: [null] });
  const { reply } = await webAx(f.carrier, {}, f.readPlain);
  expect(reply.ax.elements).toEqual([]);
  expect(reply.ax.coverage.complete).toBe(false);
  expect(reply.ax.truncated.elements).toBe('unknown');
  expect(axClean(reply)).toBe(false);
});

test('tree --ax: a target inside a modal is judged against the whole modal, not reported outside it', () => {
  const dialog = el(3, 'dialog', 'Sheet'), ok = el(4, 'button', 'Ok');
  dialog.i = 0; ok.i = 1; ok.parent = 0;
  const r = axFinish(axReply([dialog, ok], { modal: { present: true, element: 0, id: 13, by: 'dialog:modal' } }), axPlain, 14);
  expect(r.ax.elements.map(e => e.testId)).toEqual(['ok']);
  expect(r.ax.findings).toEqual([]);
});

test('tree --ax: the reply fits 256 KB of UTF-8, and its intent and findings cover only the elements it keeps', () => {
  const many = Array.from({ length: 2000 }, (_, i) => ({ ...el(0, 'button', ''), i, id: 10, description: 'é'.repeat(200) }));
  const r = axFinish(axReply(many), axPlain, null);
  expect(Buffer.byteLength(JSON.stringify(r))).toBeLessThanOrEqual(256 * 1024);
  expect(r.ax.elements.length).toBeLessThan(2000);
  expect(r.ax.findings.length).toBe(r.ax.elements.length);
  expect(r.ax.truncated.elements).toBe(2000 - r.ax.elements.length);
  expect(r.ax.coverage.complete).toBe(false);
});

test('tree --ax: parity reports a state one side can observe and does not, before comparing values', () => {
  const web = axReply([el(0, 'heading', 'Section', { testId: 'h', states: { level: 2 } }), el(1, 'checkbox', 'C', { testId: 'c', states: { checked: false } })]);
  const mac = axReply([el(0, 'heading', 'Section', { testId: 'h', states: {}, native: { role: 'AXHeading' } }), el(1, 'checkbox', 'C', { testId: 'c', states: {}, native: { role: 'AXCheckBox' } })], { source: 'appkit' });
  expect(axParity(web, mac).findings.map(f => f.detail)).toEqual(['level missing on appkit (2 vs —)', 'checked missing on appkit (false vs —)']);
  // disabled is reported only when true: its absence on both sides agrees.
  expect(axParity(web, axReply([el(0, 'heading', 'Section', { testId: 'h', states: { level: 2 } }), el(1, 'checkbox', 'C', { testId: 'c', states: { checked: false } })])).findings).toEqual([]);
});

test('tree --ax: an aria-pressed toggle is a button on every source', () => {
  const web = axReply([el(0, 'button', 'Shown', { testId: 'pressed', states: { pressed: true } })]);
  const mac = axReply([el(0, 'checkbox', 'Shown', { testId: 'pressed', states: { pressed: true }, native: { role: 'AXCheckBox', subrole: 'AXToggle' } })], { source: 'appkit' });
  const ios = axReply([el(0, 'toggleButton', 'Shown', { testId: 'pressed', value: '1', native: { role: ['toggleButton'] } })], { source: 'uikit' });
  expect(axParity(web, mac).findings).toEqual([]);
  expect(axParity(web, ios).findings).toEqual([]);
});

test('tree --ax: parity compares expanded where a side reports it, and AppKit\'s true-only report', () => {
  const web = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true } }), el(1, 'button', 'Other', { testId: 'other', states: { expanded: false } })]);
  const ios = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: false }, native: { role: ['button'] } }), el(1, 'button', 'Other', { testId: 'other', states: { expanded: false }, native: { role: ['button'] } })], { source: 'uikit' });
  expect(axParity(web, ios).findings.map(f => f.detail)).toEqual(['expanded true (web) vs false (uikit)']);
  const mac = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: 'AXButton' } }), el(1, 'button', 'Other', { testId: 'other', native: { role: 'AXButton' } })], { source: 'appkit' });
  expect(axParity(web, mac).findings.map(f => f.detail)).toEqual(['expanded true (web) vs not reported (appkit)']);
  const agree = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true }, native: { role: 'AXButton' } }), el(1, 'button', 'Other', { testId: 'other', native: { role: 'AXButton' } })], { source: 'appkit' });
  expect(axParity(web, agree).findings).toEqual([]);
});

test('tree --ax: sixty long guest-frame URLs still leave a reply within 256 KB, their truncation counted', async () => {
  const docs = Array.from({ length: 61 }, (_, k) => ({ documentURL: k + 2, nodes: { backendNodeId: [], attributes: [], parentIndex: [], nodeType: [], nodeName: [] }, layout: { nodeIndex: [], bounds: [] } }));
  const strings = ['data-agent-view', '1', ...Array.from({ length: 61 }, (_, k) => `https://example.com/${k}/` + 'x'.repeat(5000))];
  docs[0] = { nodes: { backendNodeId: [100], attributes: [[0, 1]], parentIndex: [-1], nodeType: [1], nodeName: [0] }, layout: { nodeIndex: [], bounds: [] } };
  const snapshot = { strings, documents: docs };
  const carrier = { browser: 'chrome', axEnabled: 'Chrome 1', async evaluate() { return 'Chrome/1'; },
    async call(m) { return m === 'Accessibility.getFullAXTree' ? { nodes: [{ nodeId: 'r', role: { value: 'RootWebArea' }, childIds: ['b'] }, button] } : snapshot; },
    async ask() { return { nonce: 1, epoch: 1, incarnation: 1, clock: 0 }; } };
  const { reply } = await webAx(carrier, {}, async () => ({ epoch: 1, incarnation: 1, nodes: [{ id: 1, props: {}, children: [] }] }));
  const r = axFinish(reply, { nodes: [{ id: 1, props: {}, children: [] }] }, null);
  expect(Buffer.byteLength(JSON.stringify(r))).toBeLessThanOrEqual(256 * 1024);
  expect(r.ax.coverage.excluded.length).toBe(8);
  expect(r.ax.coverage.excludedMore).toBe(52);
  expect(r.ax.coverage.excluded.every(x => x.frame.length <= 200)).toBe(true);
  expect(r.ax.coverage.complete).toBe(false);
  expect(r.ax.elements.length).toBe(1);
  // And a reply whose metadata alone is too big gives the metadata up, counted, never the bound.
  const huge = axReply([], { coverage: { roots: ['document'], complete: true, visited: 0, excluded: Array.from({ length: 60 }, () => ({ frame: 'x'.repeat(5000), reason: 'r' })) } });
  const h = axFinish(huge, { nodes: [] }, null);
  expect(Buffer.byteLength(JSON.stringify(h))).toBeLessThanOrEqual(256 * 1024);
  expect(h.ax.truncated.excluded).toBe(60);
});

test('tree --ax: the host resolves the target (the active route\'s view first), and the reply is scoped by that id', async () => {
  const plain = { epoch: 1, incarnation: 1, roots: [1], nodes: [
    { id: 1, parent: null, props: {}, children: [2, 3] }, { id: 2, parent: 1, props: { testId: 'dup' }, inactive: true, children: [] }, { id: 3, parent: 1, props: { testId: 'dup' }, children: [] }] };
  const native = { epoch: 1, incarnation: 1, ax: { source: 'uikit', order: 'containment', coverage: { roots: [], complete: true, visited: 3 }, modal: { present: false },
    elements: [{ ...el(0, 'button', 'Covered'), id: 2 }, { ...el(1, 'button', 'Showing'), i: 1, id: 3 }] } };
  const asked = [];
  const s = { carrier: { host: 'ios' }, async op(req) { asked.push(req); if (req.ax) return structuredClone(native); if (req.target === 'dup') return { ...plain, roots: [3] }; return req.target != null ? plain : structuredClone(plain); } };
  const r = await axTree(s, 'dup');
  expect(r.ax.elements.map(e => e.name)).toEqual(['Showing']);
  expect(r.ax.target).toBe(3);
  expect(asked.findIndex(q => q.target === 'dup')).toBeLessThan(asked.findIndex(q => q.ax)); // resolved inside the bracket, before the read
});

test('tree --ax: a 70-deep inert ancestor still hides, and targeting it keeps its deepest descendant', () => {
  const nodes = Array.from({ length: 71 }, (_, k) => ({ id: k + 1, parent: k ? k : null, props: k === 0 ? { inert: true } : k === 70 ? { testId: 'deep' } : {}, children: k < 70 ? [k + 2] : [] }));
  const reply = axReply([{ ...el(0, 'button', 'Deep'), id: 71 }]);
  const r = axFinish(reply, { nodes }, null);
  expect(r.ax.findings.map(f => [f.kind, f.under])).toEqual([['exposed-hidden', 1]]);
  expect(axClean(r)).toBe(true); // complete, and the finding is reported
  const t = axFinish(axReply([{ ...el(0, 'button', 'Deep'), id: 71 }]), { nodes }, 1);
  expect(t.ax.elements.map(e => e.testId)).toEqual(['deep']);
});

test('tree --ax: parity skips a state the runtime cannot observe (UIKit expanded before iOS 18)', () => {
  const web = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', states: { expanded: true } })]);
  const ios17 = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: ['button'] } })], { source: 'uikit', observes: ['checked', 'disabled'] });
  expect(axParity(web, ios17).findings).toEqual([]);
  const ios18 = axReply([el(0, 'button', 'Toggle', { testId: 'toggle', native: { role: ['button'] } })], { source: 'uikit', observes: ['checked', 'disabled', 'expanded'] });
  expect(axParity(web, ios18).findings.map(f => f.detail)).toEqual(['expanded missing on uikit (true vs —)']);
});
