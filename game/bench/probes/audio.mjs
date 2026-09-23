// Bench-only live WebAudio observation. Injected by CDP, never shipped in the module.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { Cdp } from '../../../scripts/agent.mjs';
import { serveStatic } from '../../../host/web/serve.mjs';

function observe() {
  const contexts = [], gestures = [];
  const raf = window.requestAnimationFrame.bind(window);
  let frameCallbacks = 0;
  window.requestAnimationFrame = callback => raf(at => { frameCallbacks++; callback(at); });
  for (const type of ['keydown', 'pointerdown']) window.addEventListener(type, event => {
    const gesture = {type, trusted:event.isTrusted, at:performance.now(), frameCallbacks};
    gestures.push(gesture);
    raf(() => { gesture.inputToFrameMs = performance.now() - gesture.at; });
  }, true);
  const Native = window.AudioContext;
  window.AudioContext = new Proxy(Native, { construct(Target, args) {
    const start = performance.now();
    const context = new Target(...args), constructionMs = performance.now() - start, analyser = context.createAnalyser();
    analyser.fftSize = 2048;
    analyser.connect(context.destination);
    const record = {context, analyser, constructionMs, createdFrame:frameCallbacks, createdTrusted:window.event?.isTrusted === true, resumes:[], sources:[], refusedStarts:0};
    contexts.push(record);
    const resume = context.resume.bind(context);
    context.resume = () => { record.resumes.push({trusted:window.event?.isTrusted === true && ['keydown', 'pointerdown'].includes(window.event.type), at:performance.now()}); return resume(); };
    // Each voice ends in a channel merger; route it through the analyser.
    const merger = context.createChannelMerger.bind(context);
    context.createChannelMerger = (...args) => {
      const node = merger(...args), connect = node.connect.bind(node);
      node.connect = (destination, ...rest) => connect(destination === context.destination ? analyser : destination, ...rest);
      return node;
    };
    const source = context.createBufferSource.bind(context);
    context.createBufferSource = () => {
      if (window.audioRefuseNextStart) {
        window.audioRefuseNextStart = false;
        record.refusedStarts++;
        throw new DOMException('injected start failure', 'NotSupportedError');
      }
      const node = source(), start = node.start.bind(node);
      node.start = (...args) => {
        record.sources.push({looping:node.loop, samples:node.buffer?.length ?? 0,
          channels:node.buffer?.numberOfChannels ?? 0, sampleRate:node.buffer?.sampleRate ?? 0});
        return start(...args);
      };
      return node;
    };
    return context;
  }});
  window.audioProof = () => ({
    gestures, frameCallbacks,
    contexts:contexts.map(({context, analyser, createdTrusted, constructionMs, createdFrame, resumes, sources, refusedStarts}) => {
      const samples = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(samples);
      return {createdTrusted, constructionMs, createdFrame, state:context.state, time:context.currentTime, resumes, sources, refusedStarts,
        rms:Math.sqrt(samples.reduce((sum, n) => sum + n*n, 0) / samples.length)};
    }),
    audio: (() => {
      const canvas = document.querySelector('[data-gpu-input]');
      return canvas && window.exact?.gpu?.agent(Number(canvas.dataset.view), {op:'state'})?.world?.audio;
    })(),
  });
}

/** Returns the second context's last sample and every sample, for game-specific checks. */
export async function audioProof({out, check, say, game = 'greybox', loop = 'wind', connect = child => new Cdp(child.stdio[3], child.stdio[4]), spawnBrowser = spawn}) {
  const dist = resolve(import.meta.dir, '../../games', game, 'dist');
  let server, profile, child, cdp, exited;
  const errors = [];
  const kill = () => { if (child?.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} } };
  try {
    server = createServer((req, res) => serveStatic(dist, req, res));
    await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
    profile = mkdtempSync(resolve(tmpdir(), `${game}-audio-`));
    const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
    child = spawnBrowser(chrome, ['--remote-debugging-pipe', `--user-data-dir=${profile}`,
      '--no-first-run', '--no-default-browser-check', '--disable-background-networking',
      '--disable-component-update', '--window-size=1280,720', 'about:blank'],
    {detached:true, stdio:['ignore','ignore','pipe','pipe','pipe']});
    exited = new Promise(ok => {
      child.on('exit', () => { cdp?.fail('owned Chrome exited'); ok(); });
      child.on('error', error => { cdp?.fail(error.message); ok(); });
    });
    child.stderr.on('data', data => errors.push(String(data)));
    cdp = connect(child);
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
    // Normal live ordering: allow the host to finish real frames before input.
    await until('audioProof().contexts.length === 1 && audioProof().frameCallbacks > audioProof().contexts[0].createdFrame + 1');
    const beforeGesture = await evaluate('audioProof()');
    check('completed live frame constructs before the first canvas gesture', !beforeGesture.contexts[0].createdTrusted && beforeGesture.contexts[0].resumes.length === 0, beforeGesture);
    await key('KeyW', 'w', 87);
    await until('audioProof().contexts[0]?.state === "running"');
    const afterGesture = await evaluate('audioProof()');
    check('frame-first resume runs exactly once on the trusted gesture stack', afterGesture.contexts[0].resumes.length === 1 && afterGesture.contexts[0].resumes[0].trusted, afterGesture);
    await key('KeyW', 'w', 87);
    check('later gestures do not resume a running output again', (await evaluate('audioProof()')).contexts[0].resumes.length === 1);

    // Create a fresh production surface during window capture of the next real
    // key. Its host bubble listener receives that same trusted event before any
    // frame can run. No rAF or ResizeObserver callback is withheld.
    await evaluate(`window.addEventListener('keydown', () => {
      const host = document.querySelector('[data-gpu-input]');
      const view = Number(host.dataset.view);
      const args = exact.gpu.agent(view, {op:'state'}).world.args;
      window.audioRefuseNextStart = true;
      exact.gpu.destroy(view);
      exact.gpu.surface(view, 'world', Object.values(args));
      host.focus();
    }, {capture:true, once:true})`);
    await key('KeyW', 'w', 87);
    await until('audioProof().contexts.length === 2 && audioProof().contexts[1].state === "running"');
    const gestureFirst = await evaluate('audioProof()');
    check('gesture-first construction and resume run on the trusted stack', gestureFirst.contexts[1].createdTrusted && gestureFirst.contexts[1].resumes.length === 1 && gestureFirst.contexts[1].resumes[0].trusted, gestureFirst);
    const samples = [];
    for (let i = 0; i < 30; i++) {
      samples.push(await evaluate('audioProof()'));
      await new Promise(ok => setTimeout(ok, 100));
    }
    const first = samples.find(s => s.contexts.length === 2)?.contexts[1];
    const last = samples.at(-1), context = last.contexts[1];
    check('a refused WebAudio start retries without trapping the module', context?.refusedStarts === 1 && context.sources.some(s => s.looping), context);
    check('audio resume was called on a trusted gesture stack', context?.resumes.some(r => r.trusted));
    check('audio context is running and its clock advances', context?.state === 'running' && context.time > first?.time + 0.1, context);
    check(`${loop} is active while analyser RMS is nonzero`, samples.some(s =>
      s.audio?.sources?.some(v => v.sound === loop && v.playing)
      && s.contexts.some(c => c.rms > 0 && c.sources.some(v => v.looping))), last);
    const unchanged = await evaluate(`(() => {
      const id = Number(document.querySelector('[data-gpu-input]').dataset.view);
      const before = exact.gpu.agent(id, {op:'state'}).world.hash;
      dispatchEvent(new PageTransitionEvent('pagehide'));
      return before === exact.gpu.agent(id, {op:'state'}).world.hash;
    })()`);
    check('pagehide leaves simulation state unchanged', unchanged);
    await until('audioProof().contexts[1]?.state === "suspended"', 3000);
    await evaluate('dispatchEvent(new PageTransitionEvent("pageshow", {persisted:true}))');
    await until('audioProof().contexts[1]?.state === "running"', 3000);
    check('persisted pageshow resumes output after suspension', (await evaluate('audioProof()')).contexts[1]?.state === 'running');
    writeFileSync(resolve(out, 'audio-web.json'), JSON.stringify({beforeGesture, afterGesture, gestureFirst, samples, errors}, null, 2) + '\n');
    say(`AUDIO gesture construction ${context.constructionMs.toFixed(3)} ms; input-to-frame ${last.gestures.filter(g => g.type === 'keydown').at(-1)?.inputToFrameMs?.toFixed(3)} ms; peak RMS ${Math.max(...samples.flatMap(s => s.contexts.map(c => c.rms))).toFixed(4)}; evidence: audio-web.json`);
    return {context, samples};
  } catch (error) {
    writeFileSync(resolve(out, 'audio-web.json'), JSON.stringify({error:String(error), errors}, null, 2) + '\n');
    throw error;
  } finally {
    // Kill the entire owned group even if setup or Browser.close failed.
    if (child) {
      const timer = setTimeout(kill, 3000);
      try { if (cdp) await cdp.send('Browser.close', {}, undefined, 3000); } catch {}
      finally { kill(); await exited; clearTimeout(timer); }
    }
    if (server) {
      server.closeAllConnections();
      await new Promise(ok => {
        const timer = setTimeout(ok, 1000);
        server.close(() => { clearTimeout(timer); ok(); });
      });
    }
    if (profile) rmSync(profile, {recursive:true, force:true});
  }
}

if (import.meta.main) {
  const game = process.argv[2] ?? 'greybox';
  const out = resolve(import.meta.dir, '../../games', game, 'artifacts');
  mkdirSync(out, {recursive:true});
  await audioProof({game, out, say:console.log, check(name, ok, detail) {
    if (!ok) throw new Error(`${name}: ${JSON.stringify(detail)}`);
    console.log(`PASS ${name}`);
  }});
}
