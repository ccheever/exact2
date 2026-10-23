import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {open} from './agent.mjs';
import {launchFacts} from './agent-launch.mjs';

test('a native missing target names the requests in its tree without another transport read', async () => {
  let pending = [{name:'added'}, {name:'entries'}];
  const requests = [];
  const carrier = {host:'ios', browser:'chrome', launchFacts:launchFacts({}), reset() {}, async ask(req) {
    requests.push(req.op);
    assert.equal(req.op,'tree','a miss must not issue a state request');
    return {nodes:[], pending};
  }};
  const s = await open({reuse:carrier});
  await assert.rejects(() => s.find('new-row'), /2 requests are still in flight \(added, entries\).*clock data/);
  pending = [];
  await assert.rejects(() => s.find('new-row'), e => /no view matches/.test(e.message) && !/still in flight/.test(e.message));
  assert.ok(requests.every(op => op === 'tree'));
});
