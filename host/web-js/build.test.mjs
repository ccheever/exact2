import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { webCompiler } from './module.mjs';

test('production lazy host modules ignore agent storage and entropy parameters', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/web-js-agent-'));
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Agent gate', id: 'test.agent-gate', app: { id: 'test.agent-gate', name: 'Agent gate' } }));
    writeFileSync(resolve(dir, 'app.contract'), 'component App\n  view\n    input type="file"\n');
    const build = (out, args = []) => {
      const result = spawnSync(process.execPath, ['host/web-js/build.mjs', 'agent-gate', '--out', out, '--render', 'none', ...args], {
        cwd: root, env: { ...process.env, EXACT_APP_DIR: dir }, encoding: 'utf8',
      });
      expect(result.status, result.stdout + result.stderr).toBe(0);
    };
    const dev = resolve(dir, 'dev'), prod = resolve(dir, 'prod');
    build(dev);
    build(prod, ['--production', '--plan', resolve(dev, 'app.plan')]);
    const development = await import(pathToFileURL(resolve(dev, 'storage-environment.js')).href);
    const production = await import(pathToFileURL(resolve(prod, 'storage-environment.js')).href);
    const appId = 'test.agent-gate', own = development.storageKey(appId, 'https://example.test/');
    expect(own).toMatch(/^test\.agent-gate@[a-f0-9]{12}$/);
    expect(development.storageKey(appId, 'https://example.test/?agent&storage=x')).toBe(`${own}/agent/x`);
    expect(development.storageKey(appId, 'https://example.test/?agent')).toBeNull();
    expect(development.agentSeed('http://localhost/?agent&seed=7')).toBe(7);
    for (const origin of ['https://example.test', 'http://localhost']) {
      for (const query of ['', '?agent', '?agent&storage=x', '?agent&storage=../invalid']) {
        expect(production.storageKey(appId, origin + '/' + query)).toBe(appId);
      }
      expect(production.agentSeed(origin + '/?agent&seed=7')).toBeNull();
    }
    const shipped = readdirSync(prod, { recursive: true }).filter(name => /\.m?js$/.test(name));
    expect(shipped).toContain('picker-glue.js');
    expect(shipped).toContain('storage-environment.js');
    expect(shipped).toContain('navigation.js');
    for (const name of shipped) {
      expect(readFileSync(resolve(prod, name), 'utf8'), name).not.toContain('const AGENT_ADMITTED = true;');
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}, 30000);
