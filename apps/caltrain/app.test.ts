import { expect, test } from 'bun:test';
import { open } from '../../scripts/agent.mjs';
import { checkOp } from '../../scripts/agent-check.mjs';

// Build first: bun host/web/build.mjs caltrain.
test('the plain home and both decks stay readable in both schemes', async () => {
  const app = await open({ host: 'web', app: 'caltrain' });
  const findings: string[] = [];
  const check = async (screen: string) => {
    const result = await checkOp(app, ['both']);
    for (const run of result.check) {
      for (const finding of run.findings) findings.push(`${screen} ${run.scheme}: ${finding.line}`);
    }
  };
  try {
    await app.tap('open-deck');
    await check('deck over sky');
    await app.tap('deck-back');
    await app.tap('sky-toggle');
    await check('plain home');
    await app.tap('deck-toggle');
    await check('plain departure deck');
    await app.tap('open-deck');
    await check('plain deck');
    expect(findings).toEqual([]);
  } finally {
    await app.close();
  }
}, 30_000);
