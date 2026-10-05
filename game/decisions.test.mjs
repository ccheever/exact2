import {test, expect} from 'bun:test';
import {mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {decide} from './proof.mjs';

const question = {key:'test-secret', state:{health:50, tick:120},
  goal:'Survive', choices:{eat:'Eat carried food', flee:'Return to camp'}};

test('Jev sees only the supplied observation and returns a named action with evidence', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-decisions-'));
  const transcript = join(dir, 'decisions.jsonl');
  try {
    const result = await decide({...question, transcript, fetch:async (url, options) => {
      expect(url).toBe('https://ai-gateway.vercel.sh/v1/evaluate');
      expect(options.headers.authorization).toBe('Bearer test-secret');
      expect(JSON.parse(options.body)).toEqual({model:'typesafe-ai/jev', state:question.state,
        questions:{action:{type:'choice', instructions:question.goal, criteria:question.choices}}});
      return Response.json({answers:{action:{choice:'eat', confidence:0.9, probabilities:{eat:0.9,flee:0.1}}}});
    }});
    expect(result.choice).toBe('eat');
    const log = readFileSync(transcript, 'utf8');
    expect(log).not.toContain('test-secret');
    expect(JSON.parse(log).request.state.tick).toBe(120);
    expect(JSON.parse(log).probabilities.eat).toBe(0.9);
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('Jev refuses unknown or malformed decisions and does not select a fallback', async () => {
  for (const reply of [{}, {answers:{action:{choice:'teleport'}}}, {answers:{action:{choice:'toString'}}}])
    await expect(decide({...question, fetch:async () => Response.json(reply)})).rejects.toThrow('no allowed action');
});

test('missing credentials and oversized observations fail before network I/O', async () => {
  let calls = 0;
  const fetch = async () => { calls++; return Response.json({}); };
  await expect(decide({...question, key:' ', fetch})).rejects.toThrow('AI_GATEWAY_API_KEY');
  await expect(decide({...question, state:'x'.repeat(65536), fetch})).rejects.toThrow('64 KiB');
  expect(calls).toBe(0);
});

test('invalid playtest setup names every offending action before making a request', async () => {
  let calls = 0;
  const fetch = async () => { calls++; return Response.json({}); };
  const fail = await decide({...question, goal:' ', choices:{NW:'Walk northwest', wait:''}, fetch})
    .then(()=>null, error=>error.message);
  expect(fail).toContain('goal must be a nonempty string');
  expect(fail).toContain('choice "NW" has an invalid id');
  expect(fail).toContain('lowercase letter');
  expect(fail).toContain('choice "wait" needs a nonempty description');
  await expect(decide({...question, choices:{}, fetch})).rejects.toThrow('at least one action');
  expect(calls).toBe(0);
});

test('transport and HTTP errors never expose headers or provider bodies', async () => {
  await expect(decide({...question, fetch:async () => {throw new Error('Bearer test-secret');}}))
    .rejects.toThrow('Jev request failed or timed out');
  await expect(decide({...question, fetch:async () => new Response('test-secret', {status:401})}))
    .rejects.toThrow('Jev HTTP 401');
});
