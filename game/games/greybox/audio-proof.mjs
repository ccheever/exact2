// Bench-only live WebAudio observation. Injected by CDP, never shipped in the module.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { serveStatic } from '../../../host/web/serve.mjs';

function observe() {
  const contexts = [], gestures = [];
  for (const type of ['keydown', 'pointerdown']) window.addEventListener(type, event => {
    gestures.push({type, trusted:event.isTrusted});
  }, true);
  const Native = window.AudioContext;
  window.AudioContext = new Proxy(Native, { construct(Target, args) {
    const context = new Target(...args), analyser = context.createAnalyser();
    analyser.fftSize = 2048;
    analyser.connect(context.destination);
    const record = {context, analyser, resumes:[], sources:[]};
    contexts.push(record);
    const resume = context.resume.bind(context);
    context.resume = () => { record.resumes.push({trusted:window.event?.isTrusted === true && ['keydown', 'pointerdown'].includes(window.event.type), at:performance.now()}); return resume(); };
    const panner = context.createStereoPanner.bind(context);
    context.createStereoPanner = () => {
      const node = panner(), connect = node.connect.bind(node);
      node.connect = (destination, ...rest) => connect(destination === context.destination ? analyser : destination, ...rest);
      return node;
    };
    const source = context.createBufferSource.bind(context);
    context.createBufferSource = () => {
      const node = source(), start = node.start.bind(node);
      node.start = (...args) => {
        record.sources.push({looping:node.loop, samples:node.buffer?.length ?? 0});
        return start(...args);
      };
      return node;
    };
    return context;
  }});
  window.audioProof = () => ({
    gestures,
    contexts:contexts.map(({context, analyser, resumes, sources}) => {
      const samples = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(samples);
      return {state:context.state, time:context.currentTime, resumes, sources,
        rms:Math.sqrt(samples.reduce((sum, n) => sum + n*n, 0) / samples.length)};
    }),
    audio: (() => {
      const canvas = document.querySelector('[data-gpu-input]');
      return canvas && window.exact?.gpu?.agent(Number(canvas.dataset.view), {op:'state'})?.world?.audio;
    })(),
  });
}

export async function audioProof({out, check, say}) {
  const dist = resolve(import.meta.dir, 'dist');
  const server = createServer((req, res) => serveStatic(dist, req, res));
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const profile = mkdtempSync(resolve(tmpdir(), 'greybox-audio-'));
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const child = spawn(chrome, ['--remote-debugging-pipe', `--user-data-dir=${profile}`,
    '--no-first-run', '--no-default-browser-check', '--disable-background-networking',
    '--disable-component-update', '--window-size=1280,720', 'about:blank'],
  {stdio:['ignore','ignore','pipe','pipe','pipe']});
  const cdp = new Cdp(child.stdio[3], child.stdio[4]);
  const exited = new Promise(ok => {
    child.on('exit', () => { cdp.fail('owned Chrome exited'); ok(); });
    child.on('error', error => { cdp.fail(error.message); ok(); });
  });
  const errors = [];
  child.stderr.on('data', data => errors.push(String(data)));
  try {
    const {targetInfos} = await cdp.send('Target.getTargets');
    const target = targetInfos.find(t => t.type === 'page');
    const {sessionId} = await cdp.send('Target.attachToTarget', {targetId:target.targetId, flatten:true});
    const call = (method, params) => cdp.send(method, params, sessionId);
    cdp.listeners.push(message => {
      if (message.method === 'Runtime.exceptionThrown' || message.method === 'Runtime.consoleAPICalled') errors.push(message.params);
    });
    await call('Runtime.enable');
    await call('Page.enable');
    const evaluate = async expression => {
      const reply = await call('Runtime.evaluate', {expression, returnByValue:true, awaitPromise:true});
      if (reply.exceptionDetails) throw new Error(JSON.stringify(reply.exceptionDetails));
      return reply.result.value;
    };
    const until = async (expression, timeout = 30000) => {
      const deadline = Date.now() + timeout;
      do {
        if (await evaluate(expression)) return;
        await new Promise(ok => setTimeout(ok, 100));
      } while (Date.now() < deadline);
      throw new Error(`audio probe timed out: ${expression}`);
    };
    await call('Page.addScriptToEvaluateOnNewDocument', {source:`(${observe.toString()})()`});
    await call('Page.navigate', {url:`http://127.0.0.1:${server.address().port}/`});
    await call('Page.bringToFront');
    await until('!!document.querySelector("[data-testid=play]")');
    await until('typeof window.audioProof === "function"', 3000);
    await evaluate('exact.ready');
    check('audio bench uses the live host clock', await evaluate('!exact.now && !exact.agent'));
    await evaluate('document.querySelector("[data-testid=play]").focus()');
    const key = async (code, key, vk) => {
      await call('Input.dispatchKeyEvent', {type:'keyDown', code, key, windowsVirtualKeyCode:vk});
      await call('Input.dispatchKeyEvent', {type:'keyUp', code, key, windowsVirtualKeyCode:vk});
    };
    const play = await evaluate('(() => { const r = document.querySelector("[data-testid=play]").getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()');
    await call('Input.dispatchMouseEvent', {type:'mousePressed', ...play, button:'left', clickCount:1});
    await call('Input.dispatchMouseEvent', {type:'mouseReleased', ...play, button:'left', clickCount:1});
    await until('!!document.querySelector("[data-gpu-input]") && !!exact.gpu?.wantsInput(Number(document.querySelector("[data-gpu-input]").dataset.view))');
    await evaluate('document.querySelector("[data-gpu-input]").focus()');
    await key('KeyW', 'w', 87);
    const samples = [];
    for (let i = 0; i < 30; i++) {
      samples.push(await evaluate('audioProof()'));
      await new Promise(ok => setTimeout(ok, 100));
    }
    const first = samples.find(s => s.contexts.length)?.contexts[0];
    const last = samples.at(-1), context = last.contexts[0];
    check('audio resume was called on a trusted gesture stack', context?.resumes.some(r => r.trusted));
    check('audio context is running and its clock advances', context?.state === 'running' && context.time > first?.time + 0.1, context);
    check('wind is active while analyser RMS is nonzero', samples.some(s =>
      s.audio?.sources?.some(v => v.sound === 'wind' && v.playing)
      && s.contexts.some(c => c.rms > 0 && c.sources.some(v => v.looping))), last);
    writeFileSync(resolve(out, 'audio-web.json'), JSON.stringify({samples, errors}, null, 2) + '\n');
    say('AUDIO live browser evidence: audio-web.json');
  } catch (error) {
    writeFileSync(resolve(out, 'audio-web.json'), JSON.stringify({error:String(error), errors}, null, 2) + '\n');
    throw error;
  } finally {
    const killTimer = setTimeout(() => child.kill('SIGKILL'), 3000);
    try { await cdp.send('Browser.close', {}, undefined, 3000); } catch { child.kill('SIGKILL'); }
    await exited;
    clearTimeout(killTimer);
    await new Promise(ok => server.close(ok));
    rmSync(profile, {recursive:true, force:true});
  }
}
