import {test, expect} from 'bun:test';
import {EventEmitter} from 'node:events';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {decide, finishReplay, loadReplay, observationChanges, replayOutcome, ReplayDivergence} from './proof.mjs';

// No driver environment leaks into these: each source is named or keyed here.
const question = {key:'test-secret', env:{}, state:{health:50, tick:120},
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

test('without a source, the refusal names every variable that supplies one', async () => {
  const fail = await decide({...question, key:undefined}).then(()=>null, error=>error.message);
  for (const name of ['AI_GATEWAY_API_KEY', 'ANTHROPIC_API_KEY', 'EXACT_JEV_SOURCE=claude', '--replay']) expect(fail).toContain(name);
  await expect(decide({...question, source:'anthropic'})).rejects.toThrow('ANTHROPIC_API_KEY');
  await expect(decide({...question, env:{EXACT_JEV_SOURCE:'oracle'}})).rejects.toThrow('not a Jev source');
});

test('the Messages API reads the same goal, observation and choices, and answers one of them', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-decisions-'));
  const transcript = join(dir, 'decisions.jsonl');
  try {
    const result = await decide({...question, key:undefined, env:{ANTHROPIC_API_KEY:'test-secret'}, transcript,
      fetch:async (url, options) => {
        expect(url).toBe('https://api.anthropic.com/v1/messages');
        expect(options.headers['x-api-key']).toBe('test-secret');
        const body = JSON.parse(options.body);
        expect(body.model).toBe('claude-opus-5-5');
        expect(JSON.parse(body.messages[0].content)).toEqual({goal:question.goal, state:question.state, choices:question.choices});
        expect(body.output_config.format.schema.properties.choice.enum).toEqual(['eat', 'flee']);
        return Response.json({stop_reason:'end_turn', content:[{type:'text', text:'{"choice":"flee","confidence":0.7}'}],
          usage:{input_tokens:200, output_tokens:12}});
      }});
    expect(result.choice).toBe('flee');
    const row = JSON.parse(readFileSync(transcript, 'utf8'));
    expect(row.source).toEqual({name:'anthropic', model:'claude-opus-5-5'});
    expect(row.request.questions.action.criteria).toEqual(question.choices);
    expect(JSON.stringify(row)).not.toContain('test-secret');
    await expect(decide({...question, key:undefined, env:{ANTHROPIC_API_KEY:'k'},
      fetch:async () => Response.json({stop_reason:'refusal', content:[]})})).rejects.toThrow('Jev refused');
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('the Claude CLI source runs outside the repository with the same request', async () => {
  let seen;
  const spawn = (command, args, options) => {
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.stdin = {end:input => queueMicrotask(() => {
      seen = {command, args, cwd:options.cwd, input:JSON.parse(input)};
      child.stdout.emit('data', JSON.stringify({is_error:false, structured_output:{choice:'eat', confidence:0.8}, usage:{input_tokens:9, output_tokens:3}}));
      child.emit('close', 0);
    })};
    child.kill = () => {};
    return child;
  };
  const result = await decide({...question, key:undefined, env:{EXACT_JEV_SOURCE:'claude', EXACT_JEV_MODEL:'claude-haiku-4-5'}, spawn});
  expect(result.choice).toBe('eat');
  expect(seen.command).toBe('claude');
  expect(seen.args).toContain('claude-haiku-4-5');
  expect(seen.cwd).toBe(tmpdir());
  expect(seen.input.choices).toEqual(question.choices);
});

const recorded = (choice, state, choices = question.choices) => ({request:{model:'typesafe-ai/jev', state,
  questions:{action:{type:'choice', instructions:'Survive', criteria:choices}}}, choice});

test('replay feeds recorded actions by id, separating changed words from changed numbers', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-replay-'));
  try {
    const file = join(dir, 'jev-decisions.jsonl');
    writeFileSync(file, [recorded('eat', {health:'Health 50', note:'Hungry'}), recorded('flee', {health:'Health 60', recent:[1]})]
      .map(row => JSON.stringify(row)).join('\n') + '\n');
    const replay = loadReplay(dir, {arm:false});
    let calls = 0;
    const fetch = async () => { calls++; return Response.json({}); };
    const first = await decide({...question, key:undefined, replay, fetch, state:{health:'Health: 50', note:'Starving'}});
    expect(first.choice).toBe('eat');
    const second = await decide({...question, key:undefined, replay, fetch, state:{health:'Health 70', recent:[2]}});
    expect(second.choice).toBe('flee');
    expect(calls).toBe(0);
    expect(replay.steps[0].changes.map(c => [c.path, c.kind])).toEqual([['state.health', 'copy'], ['state.note', 'copy']]);
    expect(replay.steps[1].changes.map(c => [c.path, c.kind])).toEqual([['state.health', 'value']]);
    const error = await decide({...question, key:undefined, replay, state:{}}).then(()=>null, e=>e);
    expect(error).toBeInstanceOf(ReplayDivergence);
    expect(error.message).toContain('decision 3');
    expect(error.message).toContain('first value divergence at decision 2');
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('replay stops where the build no longer offers the recorded action', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-replay-'));
  try {
    const file = join(dir, 'jev-decisions.jsonl');
    writeFileSync(file, JSON.stringify(recorded('eat', {food:'1 food'})) + '\n');
    const replay = loadReplay(file, {arm:false});
    const message = await decide({...question, key:undefined, replay, state:{food:'0 food'}, choices:{flee:'Return to camp'}})
      .then(()=>null, e=>e.message);
    expect(message).toContain('recorded action "eat" is not offered (offered: flee)');
    expect(message).toContain('value state.food: "1 food" → "0 food"');
    const lines = [], failures = [];
    const report = finishReplay(replay, {say:line => lines.push(line), check:(label, ok) => { if (!ok) failures.push(label); },
      out:dir, interrupted:true});
    expect(report.diverged).toBe(1);
    expect(failures).toEqual([]);
    expect(JSON.parse(readFileSync(join(dir, 'replay.json'), 'utf8')).firstValueDivergence).toBe(1);
  } finally { rmSync(dir, {recursive:true,force:true}); }
});

test('outcomes agree on their own values and tick; a rewritten scene is a note', () => {
  const world = (hash, x = 1, tick = 10) => ({tick, hash, entities:[{id:1, name:'player', components:{P:{x}}}]});
  expect(replayOutcome({purse:'20¢', world:world('0x1')}, {purse:'20 coins', world:world('0x1')}))
    .toMatchObject({same:true, basis:'the outcome\'s values; world tick 10, hash 0x1', notes:[expect.stringContaining('outcome wording')]});
  expect(replayOutcome({food:'Plant food · 0/3'}, {food:0})).toMatchObject({same:true, notes:[expect.stringContaining('outcome shape')]});
  expect(replayOutcome({food:'Plant food · 1/3'}, {food:0}).same).toBe(false);
  expect(replayOutcome({}, {orders:5})).toMatchObject({same:true, notes:['outcome field: added outcome.orders: 5']});
  expect(replayOutcome({pack:[{index:7, generation:0}]}, {pack:[{index:6, generation:0}]}))
    .toMatchObject({same:true, notes:[expect.stringContaining('outcome handle: handle outcome.pack.0')]});
  const scene = replayOutcome({world:world('0x1')}, {world:world('0x2', 2)});
  expect(scene.same).toBe(true);
  expect(scene.notes[0]).toContain('P ×1');
  expect(scene.world[0].path).toBe('world.player.P.x');
  expect(replayOutcome({world:world('0x1')}, {world:world('0x1', 1, 11)}).differences[0].path).toBe('world.tick');
  expect(observationChanges({a:'3 of 5'}, {a:'3 / 5', b:1})).toEqual([
    {path:'a', kind:'copy', before:'3 of 5', after:'3 / 5'}, {path:'b', kind:'added', after:1}]);
});
