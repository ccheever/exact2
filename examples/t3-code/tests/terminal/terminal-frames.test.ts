// The terminal pages' app side (terminal-frames.ts): posts wait for a page's boot and go one at a time, a new
// document drops what the old one was sent, a sign-in transcript goes again when it moves on, and a system line
// reaches every frame showing the terminal.
import { describe, expect, test } from 'bun:test';
import { postToTerminalFrame, terminalFrameMessage, terminalFrameSystemMessage, terminalFrameView } from '../../src/terminal/terminal-frames';
import { AUTH_TERMINALS, authFrameId, authTerminalFrame } from '../../src/providers/provider-auth-terminal';

const mint = async () => 'ws://host/ws?wsTicket=t';
const texts = (owner: object, id: string) => terminalFrameView(owner, id, {}).posts.map(post => JSON.parse(post.text));

describe('terminal frames', () => {
  test('a post waits for the boot; a second boot drops the first document\'s posts', async () => {
    const owner = {}, id = 'frame-a';
    postToTerminalFrame(owner, id, { type: 'system', message: 'early' });
    expect(texts(owner, id)).toEqual([]);
    await terminalFrameMessage(owner, id, { type: 'boot' }, { mint });
    expect(texts(owner, id)).toEqual([]);
    await terminalFrameMessage(owner, id, { type: 'ticket', serial: 3 }, { mint });
    postToTerminalFrame(owner, id, { type: 'focus' });
    expect(texts(owner, id)).toEqual([{ type: 'socket', url: 'ws://host/ws?wsTicket=t', serial: 3 }]);
    const [first] = terminalFrameView(owner, id, {}).posts;
    await terminalFrameMessage(owner, id, { type: 'posted', key: first!.key }, { mint });
    expect(texts(owner, id)).toEqual([{ type: 'focus' }]);
    await terminalFrameMessage(owner, id, { type: 'boot' }, { mint });
    expect(texts(owner, id)).toEqual([]);
  });

  test('a mint that fails or is let go still answers the page, with no URL, so it asks again', async () => {
    const owner = {}, id = 'frame-a';
    await terminalFrameMessage(owner, id, { type: 'boot' }, { mint });
    const letGo = Object.assign(new Error('let go'), { name: 'FetchError', kind: 'Aborted' });
    await expect(terminalFrameMessage(owner, id, { type: 'ticket' }, { mint: async () => { throw letGo; } })).rejects.toBe(letGo);
    expect(texts(owner, id)).toEqual([{ type: 'socket', url: '', serial: 0 }]);
  });

  test('the frames are kept by use: a frame in view outlives 64 newer ones', async () => {
    const owner = {};
    await terminalFrameMessage(owner, 'in-view', { type: 'boot' }, { mint });
    postToTerminalFrame(owner, 'in-view', { type: 'focus' });
    for (let index = 0; index < 100; index++) {
      terminalFrameView(owner, `other-${index}`, {});
      terminalFrameView(owner, 'in-view', {});
    }
    expect(texts(owner, 'in-view')).toEqual([{ type: 'focus' }]);
  });

  test('a delivered socket URL leaves the view at once', async () => {
    const owner = {}, id = 'frame-a';
    await terminalFrameMessage(owner, id, { type: 'boot' }, { mint });
    await terminalFrameMessage(owner, id, { type: 'ticket' }, { mint });
    const [post] = terminalFrameView(owner, id, {}).posts;
    expect(post!.text).toContain('wsTicket');
    await terminalFrameMessage(owner, id, { type: 'posted', key: post!.key }, { mint });
    expect(JSON.stringify(terminalFrameView(owner, id, {}))).not.toContain('wsTicket');
  });

  test('a system line goes to every frame the match names', async () => {
    const owner = {};
    for (const id of ['frame-a', 'frame-b']) await terminalFrameMessage(owner, id, { type: 'boot' }, { mint });
    expect(terminalFrameSystemMessage(owner, id => id === 'frame-b', 'Unable to open link')).toBe(true);
    expect(texts(owner, 'frame-a')).toEqual([]);
    expect(texts(owner, 'frame-b')).toEqual([{ type: 'system', message: 'Unable to open link' }]);
    expect(terminalFrameSystemMessage(owner, () => false, 'x')).toBe(false);
  });

  test('the sign-in terminal is the page in transcript mode; its transcript goes after the boot and again as it grows', async () => {
    const id = authFrameId('codex');
    const view = authTerminalFrame('codex', 'flow-1', 'i-1', 'Visit https://example.com\r\n', 27);
    expect(JSON.parse(view.session)).toEqual({ mode: 'transcript', identity: { authFlow: 'flow-1:i-1' }, readOnly: false, canAddToChat: false,
      auth: { instanceId: 'codex', flowId: 'flow-1', interactionId: 'i-1' } });
    expect(view.posts).toEqual([]);
    await terminalFrameMessage(AUTH_TERMINALS, id, { type: 'boot' }, { mint });
    const booted = authTerminalFrame('codex', 'flow-1', 'i-1', 'Visit https://example.com\r\n', 27);
    expect(booted.posts.map(post => JSON.parse(post.text))).toEqual([{ type: 'transcript', flow: 'flow-1:i-1', output: 'Visit https://example.com\r\n', offset: 27 }]);
    await terminalFrameMessage(AUTH_TERMINALS, id, { type: 'posted', key: booted.posts[0]!.key }, { mint });
    expect(authTerminalFrame('codex', 'flow-1', 'i-1', 'Visit https://example.com\r\n', 27).posts).toEqual([]);
    expect(authTerminalFrame('codex', 'flow-1', 'i-1', 'Visit https://example.com\r\nCode: 1234', 37).posts.map(post => JSON.parse(post.text).offset)).toEqual([37]);
  });
});
