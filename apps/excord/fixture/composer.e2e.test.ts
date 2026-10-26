import { expect, test } from 'bun:test';
import { writeFileSync, unlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runTests } from '../../../scripts/agent.mjs';
import { startFixture } from './server';

// The Windows app driver uses the prebuilt Excord named by EXACT_WINDOWS_BIN.
test.skipIf(process.platform !== 'win32' || !process.env.EXACT_WINDOWS_BIN)(
  'pending edits preserve newer text and edit sessions, and cannot be submitted twice', async () => {
    const names = ['newer text', 'reopened edit', 'reopened edit failure', 'duplicate save'];
    const file = join(tmpdir(), `excord-composer-races-${process.pid}-${Date.now()}.test.contract`);
    const setup = `  clock +500 real
  tap "channel-general"
  clock settle
  tap "mark-read"
  type "composer" "original message"
  tap "send"
  clock +350
  clock settle
  type "composer" "outer draft"
  tap "row-1562887009075210000" contextmenu
  tap "menu-reply"
  tap "row-1562888267366400000" contextmenu
  tap "menu-edit"
  type "composer" "first saved edit"
  expect state editing == "1562888267366400000"
`;
    writeFileSync(file, 'epoch "2026-10-22T18:00:00Z"\ntime-zone "America/Los_Angeles"\nsize 1100x760\n\n'
      + names.map(name => `test "${name}"\n${setup}`).join('\n'));
    let hold = false, fail = false, received = 0;
    let notified: (() => void) | null = null;
    const waiting: (() => void)[] = [];
    const release = () => { hold = false; fail = false; for (const resume of waiting.splice(0)) resume(); };
    const serve = Bun.serve;
    let fixture: ReturnType<typeof startFixture> | undefined;
    const bag = (state: any) => ({ ...state.resources, ...state.derives, ...state.slots });
    try {
      try {
        // Hold the real REST request; subsequent input is delivered without the
        // authored test clock waiting for that request to finish first.
        Bun.serve = ((options: any) => {
          const fetch = options.fetch;
          options.fetch = async (request: Request, server: any) => {
            if (request.method === 'PATCH' && /^\/api\/v9\/channels\/[^/]+\/messages\/[^/]+$/.test(new URL(request.url).pathname)) {
              received++;
              const refuse = fail;
              if (hold) await new Promise<void>(resolve => { waiting.push(resolve); notified?.(); notified = null; });
              if (refuse) return Response.json({ message: 'Edit refused by fixture', code: 0 }, { status: 500 });
            }
            return fetch(request, server);
          };
          return serve(options);
        }) as typeof Bun.serve;
        fixture = startFixture({ port: 4790, world: 'transcript', resetOnIdentify: true });
      } finally { Bun.serve = serve; }
      const result = await runTests({ host: 'windows', app: 'excord', file, atEnd: async (s: any, { name, failures }: any) => {
        if (failures.length) return { error: 'Setup failed' };
        const errors: string[] = [];
        const check = (label: string, actual: unknown, expected: unknown) => {
          if (JSON.stringify(actual) !== JSON.stringify(expected)) errors.push(`${name}: ${label}: ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`);
        };
        let timeout: ReturnType<typeof setTimeout> | undefined;
        try {
          received = 0; hold = true; fail = name.endsWith('failure');
          const reached = new Promise<void>(resolve => { notified = resolve; });
          await s.tap('send');
          await Promise.race([reached, new Promise((_, reject) => {
            timeout = setTimeout(() => reject(Error('The edit did not reach the fixture')), 5000);
          })]);
          clearTimeout(timeout);
          const pending = await s.state();
          check('one edit is held', waiting.length, 1);
          check('the edit remains pending', pending.pending?.some((p: any) => p.name === 'edited'), true);
          if (name === 'newer text') {
            await s.type('composer', 'newer unsaved text');
          } else if (name.startsWith('reopened edit')) {
            await s.type('composer', { key: 'Escape' });
            await s.tap('row-1562888267366400000', { contextmenu: true });
            await s.tap('menu-reply');
            await s.type('composer', 'new outer draft');
            await s.tap('row-1562888267366400000', { contextmenu: true });
            await s.tap('menu-edit');
            // Equal text alone cannot identify the edit session.
            await s.type('composer', 'first saved edit');
          } else {
            const node = (await s.tree()).nodes.find((n: any) => n.props?.testId === 'send');
            check('Save is disabled while pending', node?.props?.disabled === true || node?.props?.disabled === 'true', true);
            await s.type('composer', { key: 'Enter' });
            check('Enter starts no second save', (await s.state()).mutationRequests, pending.mutationRequests);
          }
          release();
          await s.clock('data');
          const state = bag(await s.state());
          check('one PATCH was sent', received, 1);
          if (name !== 'duplicate save') {
            check('the editor stays open', state.editing, '1562888267366400000');
            check('the newer edit survives', state.draft, name === 'newer text' ? 'newer unsaved text' : 'first saved edit');
            check('the reply draft stays stashed', state.stash, name === 'newer text' ? 'outer draft' : 'new outer draft');
            check('the reply target stays stashed', state.stashReplyTo, name === 'newer text' ? '1562887009075210000' : '1562888267366400000');
            check('the old response adds no error to a new edit', state.editError, '');
            if (errors.length === 0) {
              await s.tap('send');
              await s.clock('data');
              const saved = bag(await s.state());
              check('the newer edit can be saved', saved.editing, '');
              check('saving restores its own draft', saved.draft, state.stash);
              check('saving restores its own reply target', saved.replyTo, state.stashReplyTo);
              check('the server holds the newer text', fixture!.world.messages['1191168914235588608'].at(-1)!.content, state.draft);
            }
          } else {
            check('successful save closes the editor', state.editing, '');
            check('successful save restores the draft', state.draft, 'outer draft');
            check('successful save restores the reply target', state.replyTo, '1562887009075210000');
          }
          return { errors };
        } finally { clearTimeout(timeout); release(); }
      } });
      const errors = result.results.flatMap((r: any) => [...r.failures, ...(r.end?.error ? [r.end.error] : []), ...(r.end?.errors ?? [])]);
      expect(errors).toEqual([]);
      expect(result.passed).toBe(names.length);
      expect(result.results.filter((r: any) => Array.isArray(r.end?.errors))).toHaveLength(names.length);
    } finally {
      release();
      await fixture?.close();
      unlinkSync(file);
    }
  }, 45000,
);
