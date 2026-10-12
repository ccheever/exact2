// app.json keys the web manifest standard defines (scripts/app.schema.json).
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';

test('readManifest takes the web manifest\'s description', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-description-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Tips', app: { id: 'dev.exact.tips', name: 'Tips' }, description: 'Splits a bill and its tip.' }));
    assert.equal(readManifest(dir, 'tips').description, 'Splits a bill and its tip.');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('protocol handlers require one in-app placeholder and distinct schemes', async () => {
  const { readManifest } = await import('./app.mjs');
  const { mkdtempSync, writeFileSync, rmSync } = await import('node:fs');
  const { tmpdir } = await import('node:os');
  const dir = mkdtempSync(resolve(tmpdir(), 'manifest-protocol-'));
  const base = { name: 'Links', app: { id: 'dev.exact.links', name: 'Links' } };
  const save = protocol_handlers => writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ ...base, protocol_handlers }));
  try {
    save([{ protocol: 'web+links', url: '/open?u=%s' }]);
    assert.deepEqual(readManifest(dir, 'links').protocol_handlers, [{ protocol: 'web+links', url: '/open?u=%s' }]);
    for (const bad of [
      [{ protocol: 'web+links', url: '/open' }],
      [{ protocol: 'web+links', url: '/open?u=%s&again=%s' }],
      [{ protocol: 'web+links', url: '//other/open?u=%s' }],
      [{ protocol: 'web+links', url: '/open?u=%s#fragment' }],
      [{ protocol: 'web+links', url: '/open?u=%s' }, { protocol: 'WEB+LINKS', url: '/other?u=%s' }],
    ]) {
      save(bad);
      assert.throws(() => readManifest(dir, 'links'), /protocol_handlers/);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('Apple bundles register declared protocol schemes and keep their templates', async () => {
  const { macInfoPlist, infoPlist } = await import('../host/apple/build.mjs');
  const app = { id: 'dev.exact.links', displayName: 'Links', manifest: {
    protocol_handlers: [{ protocol: 'web+links', url: '/open?u=%s' }],
    host: { macos: { urlSchemes: ['other'] }, ios: { urlSchemes: ['other'] } },
  } };
  for (const plist of [macInfoPlist(app), infoPlist(app)]) {
    assert.match(plist, /<key>ExactProtocolHandlers<\/key>/);
    assert.match(plist, /<string>web\+links<\/string>/);
    assert.match(plist, /<string>other<\/string>/);
    assert.match(plist, /<string>\/open\?u=%s<\/string>/);
  }
});
