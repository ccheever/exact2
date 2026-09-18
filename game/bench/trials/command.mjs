// BRIEF command contract, composed solely from the existing Linux driver.
import { resolve } from 'node:path';
import { mkdirSync, mkdtempSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export async function createCommand({root = resolve(import.meta.dirname, '../../..'), store} = {}) {
  root = resolve(root);
  process.env.EXACT_APP_DIR = resolve(root, 'game/games/lanterns');
  process.env.EXACT_UPDATE_TRUST = 'development';
  const {open} = await import(pathToFileURL(resolve(root, 'scripts/agent.mjs')));
  const {resolveApp} = await import(pathToFileURL(resolve(root, 'scripts/app.mjs')));
  const app = resolveApp('lanterns');
  process.env.EXACT_LINUX_BIN = resolve(app.target, 'x86_64-unknown-linux-gnu/release/lanterns-linux');
  const ownedStore = !store;
  const scratch = resolve(process.env.HOME, 'lanes/gamenext/scratch/trials');
  mkdirSync(scratch, {recursive:true});
  store ??= mkdtempSync(resolve(scratch, 'command-'));
  mkdirSync(store, {recursive:true});
  let s, jump = false, act = false, held = new Set(), journal = [];
  const unavailable = new Set();
  const observe = (value, field) => {
    if (value === undefined || value === null) { unavailable.add(field); return null; }
    return value;
  };
  async function reopen(world) {
    await s?.close();
    s = await open({host:'linux', app:'lanterns', size:[1280,800], world,
      env:{EXACT_SURFACE_STORE:store, XDG_DATA_HOME:store, XDG_CACHE_HOME:store}});
    jump = act = false; held.clear(); journal = [];
    await s.clock(0);
  }
  async function input(request = {}) {
    const x = Math.max(-1, Math.min(1, Number(request.moveX) || 0));
    const z = Math.max(-1, Math.min(1, Number(request.moveZ) || 0));
    // Linux has keys but no held pointer carrier. Explicit eight-direction
    // quantization, never a physics mutation or coordinate shortcut.
    const desired = new Set();
    if (x || z) {
      const angle = Math.round(Math.atan2(z,x)/(Math.PI/4))*Math.PI/4;
      const dx = Math.round(Math.cos(angle)), dz = Math.round(Math.sin(angle));
      if (dx) desired.add(dx > 0 ? 'KeyD' : 'KeyA');
      if (dz) desired.add(dz > 0 ? 'KeyS' : 'KeyW');
    }
    for (const key of held) if (!desired.has(key)) await s.world('world').key_up(key);
    for (const key of desired) if (!held.has(key)) await s.world('world').key_down(key);
    held = desired;
    for (const [key, before, after] of [['Space',jump,request.jump === true],['KeyE',act,request.act === true]]) {
      if (before !== after) await s.type('world', {key,phase:after ? 'down' : 'up'});
    }
    jump = request.jump === true; act = request.act === true;
  }
  async function state() {
    const {world} = await s.state('world', {world:true});
    const roster = await s.state('world:*');
    if (!roster.entities || roster.truncated) throw new Error('World roster unavailable or truncated');
    const get = name => roster.entities.find(e => e.name === name)?.components ?? {};
    const player = get('player'), crate = get('crate'), fox = get('fox');
    const body = (data, field, feet = 0) => {
      const at = data.Transform?.position, velocity = (data.Character ?? data.Body)?.velocity;
      return Object.fromEntries(['x','y','z'].flatMap((k,i) => [
        [k, observe(at?.[i] === undefined ? undefined : at[i] - (i === 1 ? feet : 0),`${field}.${k}`)],
        [`v${k}`,observe(velocity?.[i],`${field}.v${k}`)],
      ]));
    };
    const lanterns = roster.entities.filter(e => e.components.Lantern).map(e => {
      const [x,y,z] = e.components.Transform.position;
      return {id:e.name,x,y,z,lit:e.components.Lantern.lit};
    });
    const logs = await s.logs();
    const lines = (logs.world ?? []).flatMap(w => w.lines ?? []);
    const events = lines.flatMap(line => {
      const tick = Number(/(?:tick[=: ]+|^\[)(\d+)/.exec(line)?.[1]);
      let type, id;
      if (/\bjump$/.test(line)) type = 'jump';
      if (/\blantern-\d+ lit$/.test(line)) { type = 'lantern-lit'; id = /lantern-\d+/.exec(line)[0]; }
      if (/\ball lanterns lit$/.test(line)) type = 'all-lanterns-lit';
      if (/\bpublish phase: .*"won"/.test(line)) type = 'win';
      if (/\bnight fell$/.test(line)) type = 'lose';
      if (/surface-save:saved/.test(line)) type = 'save';
      if (/surface-load:loaded/.test(line)) type = 'load';
      return type ? [{type,tick:observe(Number.isFinite(tick) ? tick : null,'events.tick'),...(id ? {id} : {})}] : [];
    });
    journal.push(...events); journal = journal.slice(-4096);
    let phase = world.published.phase;
    if (phase === 'title' && world.args.started) phase = 'playing';
    if (phase === 'playing' && world.paused) phase = 'paused';
    return {phase,ticks:world.tick,elapsed:world.resources.Session.elapsed/60,
      remaining:world.published.remaining,player:{...body(player,'player',.65),
        grounded:observe(player.Character?.grounded,'player.grounded'),animation:observe(fox.Animation?.clip,'player.animation')},
      crate:body(crate,'crate'),lanterns,count:lanterns.filter(l=>l.lit).length,total:lanterns.length,
      events:journal,assetReady:fox.Mesh?.Asset?.[0] === 'Fox.glb' && !!fox.Animation};
  }
  async function command(request) {
    switch (request.op) {
      case 'ready': return {ready:(await state()).assetReady,engine:'exact2-linux',version:1};
      case 'state': return state();
      case 'reset': await reopen(); return state();
      case 'start': await reopen(); await s.tap('play'); return state();
      case 'input': await input(request); return state();
      case 'step': {
        if (!Number.isSafeInteger(request.ticks) || request.ticks < 0 || request.ticks > 216000) throw new Error('ticks must be an integer from 0 to 216000');
        if ((await state()).phase === 'playing') await s.world('world').ticks(request.ticks);
        return state();
      }
      case 'pause': {
        const phase = (await state()).phase;
        if (['playing','paused'].includes(phase)) await s.tap(phase === 'paused' ? 'resume' : 'pause');
        return state();
      }
      case 'save': {
        const before = await state();
        await s.world('world').save(resolve(store,'saved.world'));
        writeFileSync(resolve(store,'saved.json'),JSON.stringify({phase:before.phase}));
        return {saved:true};
      }
      case 'load': {
        const {phase} = JSON.parse(readFileSync(resolve(store,'saved.json'),'utf8'));
        await reopen(resolve(store,'saved.world'));
        if (phase !== 'title') await s.tap('play');
        if (phase === 'paused') await s.tap('pause');
        // Restored input belongs to the saved world, not this adapter's empty
        // bookkeeping set. Release all supported keys through ordinary input.
        for (const key of ['KeyW','KeyA','KeyS','KeyD','ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space','KeyE']) await s.world('world').key_up(key);
        await input();
        return state();
      }
      default: throw new Error(`Unknown command ${request.op}`);
    }
  }
  await reopen();
  return {version:3,command,reload:reopen,tree:()=>s.tree(),raw:()=>s,unavailable,limitations:['analog input quantized to nearest eight keyboard directions; Linux held pointer unsupported','world pixels unavailable'],
    async screenshot(path) { const reply = await s.screenshot(path); return {reply,unsupported:'Linux renders Contract UI with flat world canvas; no world pixels'}; },
    async close() { await s?.close(); if (ownedStore) rmSync(store,{recursive:true,force:true}); }};
}
