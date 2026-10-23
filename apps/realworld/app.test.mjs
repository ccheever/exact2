import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp } from '../../scripts/agent-check.mjs';
import { startNet } from '../../host/web-js/conformance/net.mjs';

// Build first with `bun host/web/build.mjs realworld`.
for (const scheme of ['light', 'dark']) {
  test(`feed and article text remain readable in ${scheme} mode`, async () => {
    const net = await startNet();
    let session;
    try {
      session = await open({ host: 'web', app: 'realworld', env: net.env });
      for (const article of [false, true]) {
        if (article) await session.tap('open-how-to-learn-javascript-efficiently');
        await session.clock('data');
        expect((await session.state()).derives.current.name).toBe(article ? 'article' : 'home');
        if (article) await session.tree('article-title');
        const result = await checkOp(session, [scheme]);
        expect(result.check.flatMap(r => r.findings).filter(f => f.kind === 'contrast')).toEqual([]);
      }
      expect(net.unanswered).toEqual([]);
    } finally {
      await session?.close();
      await net.close();
    }
  }, 30000);
}
