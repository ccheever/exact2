import { readFileSync } from 'node:fs';
import { describe, expect, test } from 'bun:test';
import { answer } from './app';

describe('mobile pairing source', () => {
  test('keeps the copied parser identical to the adopted desktop source', () => {
    const local = readFileSync(new URL('./shared/r10-connect-pairing.ts', import.meta.url), 'utf8');
    const desktop = readFileSync(new URL('../r10-connect-pairing.ts', import.meta.url), 'utf8');
    expect(local.split('\n').slice(2).join('\n')).toBe(desktop);
  });
  test('uses the shared hosted-link parser', () => {
    expect(answer('pairingFields', ['https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture']))
      .toEqual({ source: 'https://t3.codes/pair?host=https%3A%2F%2Fserver.example#token=fixture', host: 'https://server.example', code: 'fixture' });
  });
  test('answers empty input at bake and rejects unknown sources', () => {
    expect(answer('pairingFields', [''])).toEqual({ source: '', host: '', code: '' });
    expect(() => answer('missing', [])).toThrow('Unknown mobile source');
  });
});

describe('generated source dispatcher', () => {
  test('unopened settings choices have a neutral first-frame result', () => {
    expect(answer('settingsChoices', ['', '', false])).toEqual({ key: '', title: '', footer: '', ready: false, options: [] });
  });
  test('scheduled task identifiers stay in the command owner, outside the UI shape', () => {
    const result = answer('automationSnapshot', [{ revision: 0 }, false]);
    expect(result.editor).not.toHaveProperty('taskId');
  });
  test('provider preparation errors stay inside the actual provider snapshot shape', () => {
    const result = answer('providerSnapshot', ['[]', true, { error: 'Cannot read providers', arbitrary: 'not a UI field' }]);
    expect(result).toEqual({ ready: false, error: 'Cannot read providers', emptyMessage: '', sections: [] });
  });
  test('stale new-task projection owners return neutral data without native calls', () => {
    let calls = 0;
    const native = { available: true, watch() {}, async later() { calls++; throw new Error('stale projection reached native'); } };
    expect(answer('voiceFocus', [true, 'Draft', 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toEqual({ owner: '', label: 'Draft' });
    expect(answer('composerAttachments', [0, 'stale-owner', 'stale-route', 0], undefined, undefined, native)).toMatchObject({ contentOwner: '', items: [], canPick: false });
    expect(answer('newTaskPrepare', ['', true, 0, true, 'e', 'p', '', 1, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ loaded: false });
    expect(answer('mediaPreview', ['', '', '', '', '', true, 0, 0, 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ ready: false, identifier: '' });
    expect(answer('attachmentDocument', ['', '', '', true, false, true, 'file', 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ ready: false });
    expect(answer('composerSettings', ['', '', false, 0, 'stale-owner', 'stale-route'], undefined, undefined, native)).toMatchObject({ open: false, canEdit: false, models: [] });
    expect(calls).toBe(0);
  });
  test('stale new-task commands and inherited property names refuse before dispatch', () => {
    expect(() => answer('command', ['draft', '', '', '', 'stale-owner', 'stale-route'])).toThrow('Choose a project');
    expect(() => answer('constructor', [])).toThrow('Unknown mobile source: constructor');
    expect(() => answer('__proto__', [])).toThrow('Unknown mobile source: __proto__');
  });
  test('browser and device source names select their exact snapshot shapes', async () => {
    const browser = await answer('browserStatus', ['browser', '', false, { revision: 0 }, 0]);
    const devices = await answer('devicesStatus', ['devices', '', false, { revision: 0 }, 0]);
    expect(browser).toHaveProperty('tabs'); expect(browser).not.toHaveProperty('devices');
    expect(devices).toHaveProperty('devices'); expect(devices).not.toHaveProperty('tabs');
  });
});
