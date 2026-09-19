// High-level world operations over the same eight host operations.
export function worldView(session, name) {
  return {
    async snapshot() {
      const {tick, hash, entities, truncated} = await session.state(`${name}:*`);
      return {tick, hash, entities, truncated};
    },
    state: entity => session.state(`${name}:${entity}`),
    save: path => session.screenshot(path, name, 'save'),
    run: ms => {
      if (!Number.isFinite(ms) || ms < 0) throw new Error('run duration must be finite and nonnegative');
      // Like Sim::run, establish the current epoch after deferred assets settle
      // before moving time. Otherwise a newly ready world can eat the first seek.
      return session.clock('+0').then(() => session.clock(`+${ms}`));
    },
    settle: async () => (await session.clock('settle')).settled === true,
    ticks: count => exactTicks(session, name, count),
    capture: (command, options = {}) => session.state(name, { ...options, world:true, capture: command }),
    async source(entity) {
      const state = await session.state(name, {world:true}), digest = state.world?.resources?.SceneIdentity?.digest;
      if (!digest) return {unavailable:'world has no authored scene identity'};
      const detail = await session.state(`${name}:${entity}`), generated = detail.entity?.components?.GeneratedBy;
      if (generated) return {generated};
      if (!session.app) return {unavailable:'session has no local source checkout'};
      const {sceneSource} = await import('../game/app/scenes.mjs');
      return sceneSource(session.app, digest, entity);
    },
    tap: code => session.type(name, {key:code}),
    key_down: code => session.type(name, {key:code, phase:'down'}),
    key_up: code => session.type(name, {key:code, phase:'up'}),
    async moveTo(entity, [x, z], {tolerance = 0.18, speed = 4.5} = {}) {
      if (![x,z,tolerance,speed].every(Number.isFinite) || tolerance <= 0 || speed <= 0)
        throw new Error('moveTo: finite target and positive finite tolerance/speed required');
      const start = await this.global_position(entity);
      if (!start) throw new Error('moveTo: position unavailable');
      const {world} = await session.state(name, {world:true});
      if (!Number.isSafeInteger(world?.hz) || world.hz < 1 || world.hz > 1000) throw new Error('moveTo: fixed-step clock unavailable');
      const stride = speed / world.hz;
      for (let burst = 0; burst < 160; burst++) {
        const at = await this.global_position(entity);
        if (!at) throw new Error("moveTo: position unavailable");
        const dx = x-at[0], dz = z-at[2];
        if (Math.hypot(dx,dz) <= tolerance) return;
        const alongX = Math.abs(dx) >= Math.abs(dz), gap = alongX ? dx : dz;
        const key = alongX ? (gap > 0 ? 'KeyD' : 'KeyA') : (gap > 0 ? 'KeyS' : 'KeyW');
        const count = Math.max(1,Math.min(10,Math.floor(Math.max(stride,Math.abs(gap)-tolerance*0.45)/stride)));
        await this.key_down(key);
        try { await this.ticks(count); } finally { await this.key_up(key); }
        await this.ticks(1);
      }
      let detail;
      try {
        const reply = await session.op({op:'layout', ...await session.target(`${name}:${entity}`), world:true, from:[start[0],start[1]+0.65,start[2]], toward:[x,start[1]+0.65,z]});
        detail = reply.error ? `diagnostic unavailable: ${reply.error}` : JSON.stringify(reply.route ?? {unavailable:'layout route missing'});
      } catch (error) { detail = `diagnostic unavailable: ${error.message}`; }
      throw new Error(`route stalled toward ${x},${z} after 160 bursts: ${detail}`);
    },
    async local_position(entity) { return (await this.get(entity, 'Transform'))?.position; },
    async global_position(entity) {
      try { return (await session.layout(`${name}:${entity}`)).entity?.world?.position; }
      catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    async get(entity, component) {
      try {
        return (await session.state(`${name}:${entity}`)).entity?.components?.[component];
      } catch (error) {
        if ((error.reply?.error === `no entity named \`${entity}\`` || error.reply?.error?.startsWith(`no entity named \`${entity}\`; `))) return undefined;
        throw error;
      }
    },
    hold: (code, ms) => session.type(name, {key:code, for:ms}),
  };
}

/** Advance an exact fixed-step count through the existing host clock, including restored epochs. */
export async function exactTicks(session, name, count) {
  if (!Number.isSafeInteger(count) || count < 0 || count > 216000) throw new Error('ticks: expected an integer from 0 to 216000');
  if (session.controlled === false) throw new Error('ticks: controlled clock required; explicitly hand off before stepping');
  const before = await session.state(name, {world:true, clockState:true}), world = before.world;
  if (!world || !Number.isSafeInteger(world.tick) || !Number.isSafeInteger(world.hz) || world.hz < 1 || world.hz > 1000)
    throw new Error(`ticks ${name}: fixed-step world clock is unavailable or unsupported`);
  if (world.paused && count) throw new Error(`ticks ${name}: world paused at tick ${world.tick}; resume its live binding first`);
  const us = world.clockState?.worldMicros, hostUs = world.clockState?.hostMicros;
  if (!Number.isSafeInteger(us) || !Number.isSafeInteger(hostUs)) throw new Error(`ticks ${name}: clock epoch unavailable; establish it with clock first`);
  const start = world.tick, target = start + count;
  const to = Math.ceil((hostUs + Math.max(0, Math.ceil(target * 1000000 / world.hz) - us)) / 1000);
  const reply = count ? await session.clock(to) : null;
  const after = await session.state(name, {world:true, clockState:true}), actual = after.world?.tick;
  const result = {world:name, requested:count, startTick:start, requestedTick:target, actualTick:actual, clock:reply?.clock ?? before.clock, hash:after.world?.hash};
  if (actual !== target) throw Object.assign(new Error(`ticks ${name}: requested ${start} → ${target}, observed ${actual}; no retry or extra frame performed`), {result});
  return result;
}
