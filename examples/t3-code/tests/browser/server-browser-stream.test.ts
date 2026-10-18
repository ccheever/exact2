import { expect, test } from 'bun:test';
import { ServerBrowserStream, streamControlLabel } from '../../src/browser/server-browser-stream';
import type { SocketHandle } from '../../src/transport/transport-session';

function fixture() {
  const writes: string[] = [], closed: boolean[] = [];
  const socket: SocketHandle = { readyState: 1, bufferedAmount: 0, send: data => writes.push(data), close: () => closed.push(true) };
  const stream = new ServerBrowserStream(); stream.begin('ws://fixture/one');
  const message = (value: unknown) => stream.event('ws://fixture/one', { type: 'message', data: JSON.stringify(value) }, socket, 100);
  const control = (controller: string, canOperate = true, dialog: unknown = null) => message({ type: 'control', controller, canOperate, generation: 1, dialog });
  return { stream, socket, writes, closed, message, control };
}

test('binary JPEG frames are ACKed while one decode and only the latest waiting frame are retained', () => {
  const { stream, socket, writes } = fixture();
  stream.event('ws://fixture/one', { type: 'open' }, socket, 0);
  for (const bytes of [[255, 216, 1, 255, 217], [255, 216, 9, 255, 217], [255, 216, 2, 255, 217]]) stream.event('ws://fixture/one', { type: 'message', data: new Uint8Array(bytes).buffer }, socket, 0);
  expect(writes).toEqual(['{"type":"ack"}', '{"type":"ack"}', '{"type":"ack"}']);
  expect([...new Uint8Array(stream.pendingJPEG()!.bytes)]).toEqual([255, 216, 1, 255, 217]);
  stream.decoded(stream.frame.decodeSequence, 'app:/tmp/first.jpg');
  stream.painted(stream.frame.image);
  expect([...new Uint8Array(stream.pendingJPEG()!.bytes)]).toEqual([255, 216, 2, 255, 217]);
  expect(stream.frame.image).toBe('app:/tmp/first.jpg'); // Stable until the next write completes.
  expect(stream.frame.connected).toBe(true);
});

test('a retired socket cannot repaint or grant input to its successor', () => {
  const { stream, socket, control, closed } = fixture(); control('you');
  stream.begin('ws://fixture/two');
  const before = stream.frame;
  stream.event('ws://fixture/one', { type: 'message', data: new Uint8Array([1]).buffer }, socket, 0);
  stream.event('ws://fixture/one', { type: 'message', data: '{"type":"control","canOperate":true,"controller":"you","generation":9,"dialog":null}' }, socket, 0);
  expect(stream.frame).toBe(before);
  expect(stream.send({ type: 'key', key: 'x' })).toBe(false);
  expect(closed).toHaveLength(1);
});

test('control permissions gate every user input, and a viewer can ask the agent for control', () => {
  const { stream, control, writes } = fixture();
  control('agent');
  expect(stream.send({ type: 'key', key: 'x' })).toBe(false);
  expect(stream.send({ type: 'takeControl' })).toBe(true);
  control('you');
  expect(stream.send({ type: 'key', action: 'down', key: 'x', text: 'x', modifiers: 0 })).toBe(true);
  expect(stream.frame).toMatchObject({ controlled: true, label: 'You have control' });
  control('you', false);
  expect(stream.send({ type: 'takeControl' })).toBe(false);
  expect(stream.send({ type: 'navigate', url: 'https://example.test/' })).toBe(false);
  expect(stream.frame.label).toBe('Read-only');
  expect(writes).toHaveLength(2);
});

test('control labels match the reference, including another viewer and unclaimed pages', () => {
  expect(streamControlLabel(null)).toBe('Connecting...');
  const { stream, control } = fixture();
  for (const [owner, label] of [['agent', 'Agent has control'], ['another-viewer', 'Another viewer has control'], ['unclaimed', 'Watching']]) {
    control(owner); expect(stream.frame.label).toBe(label);
  }
  expect(stream.frame.anotherViewer).toBe(false);
  control('another-viewer'); expect(stream.frame.anotherViewer).toBe(true);
});

test('viewport, dialog and agent pointer state retain the fields the surface draws', () => {
  const { stream, control, message } = fixture();
  message({ type: 'viewport', width: 1200, height: 800 });
  control('you', true, { type: 'prompt', message: 'Name?', defaultValue: 'Ada' });
  message({ type: 'pointer', phase: 'click', x: 40, y: 50, sequence: 2 });
  expect(stream.frame).toMatchObject({ width: 1200, height: 800, dialogType: 'prompt', dialogMessage: 'Name?', dialogDefault: 'Ada', pointerX: 40, pointerY: 50, pointerSequence: 2, pointerClick: true });
  control('you'); expect(stream.frame.dialogMessage).toBe('');
});

test('malformed messages cannot replace valid control or viewport state', () => {
  const { stream, control, message } = fixture(); control('agent');
  for (const value of [null, [], { type: 'control', controller: 'you', canOperate: true, generation: 1 }, { type: 'control', controller: 'stranger', canOperate: true, generation: 1, dialog: null }, { type: 'viewport', width: -1, height: 1 }]) message(value);
  expect(stream.frame.label).toBe('Agent has control'); expect(stream.frame.width).toBe(0);
});

test('auth refusals refresh access twice and stop after the third refusal', () => {
  const { stream, socket } = fixture();
  for (let attempt = 0; attempt < 3; attempt++) {
    if (attempt) stream.begin(`ws://fixture/${attempt}`);
    stream.event(attempt ? `ws://fixture/${attempt}` : 'ws://fixture/one', { type: 'close', code: 4401 }, socket, 100);
    expect(stream.frame.refused).toBe(attempt === 2);
  }
  stream.retry(); expect(stream.frame.refused).toBe(false); expect(stream.unauthorized).toBe(0);
});

test('normal closes back off at 500ms up to 10s and a received frame resets failures', () => {
  const { stream, socket } = fixture(); stream.event('ws://fixture/one', { type: 'open' }, socket, 0);
  stream.event('ws://fixture/one', { type: 'close', code: 1001 }, socket, 100);
  expect(stream.retryAt).toBe(600);
  stream.event('ws://fixture/one', { type: 'error' }, socket, 100);
  expect(stream.failures).toBe(1); // One failed socket can report both error and close.
  for (let index = 0; index < 10; index++) { const url = `ws://fixture/retry-${index}`; stream.begin(url); stream.event(url, { type: 'open' }, socket, 100); stream.event(url, { type: 'close', code: 1001 }, socket, 100); }
  expect(stream.retryAt).toBe(10100);
  stream.begin('ws://fixture/received');
  stream.event('ws://fixture/received', { type: 'message', data: new Uint8Array([1]).buffer }, socket, 200);
  expect(stream.failures).toBe(0);
});

test('gone and host-setup close codes stop retrying with the server instruction', () => {
  const { stream, socket } = fixture();
  stream.event('ws://fixture/one', { type: 'close', code: 4503, reason: '{"need":"libraries","command":"install browser libraries"}' }, socket, 0);
  expect(stream.frame).toMatchObject({ setupNeed: 'libraries', setupCommand: 'install browser libraries' });
  stream.retry(); stream.begin('ws://fixture/gone'); stream.event('ws://fixture/gone', { type: 'close', code: 4404 }, socket, 0);
  expect(stream.frame.gone).toBe(true);
});

test('file chooser close messages are matched to the current chooser; notices are bounded', () => {
  const { stream, message } = fixture();
  message({ type: 'fileChooser', id: 'one', multiple: true, accept: 'image/*' });
  message({ type: 'fileChooserClosed', id: 'old' }); expect(stream.frame.chooserId).toBe('one');
  message({ type: 'fileChooserClosed', id: 'one' }); expect(stream.frame.chooserId).toBe('');
  message({ type: 'clipboard', text: 'copied' }); expect(stream.frame.clipboard).toBe('copied');
  for (let i = 0; i < 100; i++) message({ type: 'popup', tabId: String(i) });
  expect(stream.notices).toHaveLength(32); expect(stream.notices.at(-1)).toEqual({ type: 'popup', tabId: '99' });
  stream.stop(); expect(stream.notices).toEqual([]);
});
