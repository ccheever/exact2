// The branch pickers' refresh (client-runtime state/vcs.ts serverRefsRevisionAtom at T3 Code 5d3d78ab93, #27438,
// #29161): after a command that can change a checkout's refs settles, the next vcs.listRefs for it bypasses the
// server's snapshot once.
import { describe, expect, test } from 'bun:test';
import { vcsRefsFreshness } from '../../src/composer/vcs-refs-fresh';

const list = (cwd = '/repo') => ({ op: 'request', method: 'vcs.listRefs', payload: { cwd, limit: 100 } });
const sent = (client: { environmentId: string }, request: Record<string, unknown>, ok = true) => {
  const fresh = vcsRefsFreshness(client, request);
  fresh.done(ok);
  return (fresh.request.payload as Record<string, unknown>).refresh === true;
};

describe('vcs.listRefs after a ref change', () => {
  test('asks refresh once after a switch, a pull request checkout or a status read settles, for that checkout only', () => {
    const client = { environmentId: 'env' };
    expect(sent(client, list())).toBe(false);
    for (const method of ['vcs.switchRef', 'git.preparePullRequestThread', 'vcs.refreshStatus']) {
      sent(client, { op: 'request', method, payload: { cwd: '/repo' } }, false);
      expect(sent(client, list('/other'))).toBe(false);
      expect(sent(client, list())).toBe(true);
      expect(sent(client, list())).toBe(false);
    }
  });
  test('a failed bypassing read asks again; another environment keeps its own state', () => {
    const client = { environmentId: 'env' };
    sent(client, { op: 'request', method: 'vcs.createRef', payload: { cwd: '/repo' } });
    expect(sent({ environmentId: 'env' }, list())).toBe(false);
    expect(sent(client, list(), false)).toBe(true);
    expect(sent(client, list())).toBe(true);
    expect(sent(client, list())).toBe(false);
    client.environmentId = 'other';
    expect(sent(client, list())).toBe(false);
  });
});
