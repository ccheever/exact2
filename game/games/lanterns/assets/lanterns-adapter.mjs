// Development-only bridge for the independent four-engine Lanterns evaluator.
// It drives Exact's real surface input, clock, save carrier, and live world state.
const PARAMS = new URLSearchParams(location.search);
const ENABLED = PARAMS.get('agent') === '1';
const STORAGE_KEY = 'lanterns-exact-world-v1';


if (ENABLED) {
  let moveDown = false;
  let jumpDown = false;
  let actDown = false;
  const adapterEvents = [];

  const exact = () => globalThis.exact;
  const tree = async () => exact()?.agent ? await exact().agent({op: 'tree'}) : null;
  async function worldView() {
    const t = await tree();
    return t?.nodes?.find(node => node.props?.testId === 'world')?.id ?? null;
  }
  async function viewByTestId(testId) {
    const t = await tree();
    const id = t?.nodes?.find(node => node.props?.testId === testId)?.id;
    return id == null ? null : exact().views.get(id);
  }
  const surface = (view, request) => exact().gpu?.agent?.(view, request) ?? null;
  const entity = (view, name) => surface(view, {op: 'state', entity: name})?.entity?.components ?? null;

  async function readyState() {
    const view = await worldView();
    if (view == null || !exact()?.gpu) return null;
    const result = surface(view, {op: 'state'})?.world;
    if (!result?.device || result.renderError) return null;
    const fox = entity(view, 'fox');
    return fox?.Mesh?.Asset?.[0] === 'fox.model' && fox?.Animation ? {view, result} : null;
  }
  async function waitReady() {
    const deadline = performance.now() + 60000;
    while (performance.now() < deadline) {
      const result = await readyState();
      if (result) return result;
      await new Promise(resolve => setTimeout(resolve, 16));
    }
    throw new Error('Exact Lanterns surface did not become ready');
  }
  function dispatchKey(host, code, down) {
    host.dispatchEvent(new KeyboardEvent(down ? 'keydown' : 'keyup', {
      bubbles: true, cancelable: true, code, key: code === 'Space' ? ' ' : code === 'KeyE' ? 'e' : code,
    }));
  }
  function dispatchPointer(host, phase, x, y) {
    host.dispatchEvent(new PointerEvent(`pointer${phase}`, {
      bubbles: true, cancelable: true, pointerId: 41003, pointerType: 'touch',
      clientX: x, clientY: y, buttons: phase === 'up' || phase === 'cancel' ? 0 : 1,
    }));
  }
  async function setInput(request = {}) {
    const {view} = await waitReady();
    const host = exact().views.get(view);
    const rect = host.getBoundingClientRect();
    const origin = {x: rect.left + Math.min(180, rect.width * .24), y: rect.top + rect.height * .5};
    const moveX = Math.max(-1, Math.min(1, Number(request.moveX) || 0));
    const moveZ = Math.max(-1, Math.min(1, Number(request.moveZ) || 0));
    if (moveX !== 0 || moveZ !== 0) {
      if (!moveDown) {
        dispatchPointer(host, 'down', origin.x, origin.y);
        moveDown = true;
      }
      dispatchPointer(host, 'move', origin.x + moveX * 60, origin.y + moveZ * 60);
    } else if (moveDown) {
      dispatchPointer(host, 'up', origin.x, origin.y);
      moveDown = false;
    }
    const jump = request.jump === true;
    const act = request.act === true;
    if (jump !== jumpDown) dispatchKey(host, 'Space', jump);
    if (act !== actDown) dispatchKey(host, 'KeyE', act);
    jumpDown = jump;
    actDown = act;
  }
  async function releaseInput() {
    await setInput({});
    moveDown = jumpDown = actDown = false;
  }
  function eventRows(lines) {
    return lines.flatMap((line) => {
      if (/\bjump$/.test(line)) return [{type: 'jump'}];
      const lit = /\b(lantern-\d+) lit$/.exec(line);
      if (lit) return [{type: 'lantern-lit', id: lit[1]}];
      if (/\ball lanterns lit$/.test(line)) return [{type: 'win'}];
      if (/\bnight fell$/.test(line)) return [{type: 'lose'}];
      return [];
    });
  }
  async function state() {
    const {view, result: world} = await waitReady();
    const player = entity(view, 'player');
    const fox = entity(view, 'fox');
    const crate = entity(view, 'crate');
    const session = world.resources?.Session ?? {};
    const published = world.published ?? {};
    const args = world.args ?? {};
    const logs = surface(view, {op: 'logs', since: 0})?.lines ?? [];
    const playerAt = player?.Transform?.position ?? [0, .65, 12];
    const playerVelocity = player?.CapsuleController?.velocity ?? [0, 0, 0];
    const crateAt = crate?.Transform?.position ?? [6, .6, 8];
    const crateVelocity = crate?.Body?.velocity ?? [0, 0, 0];
    const roster = surface(view, {op:'state', entity:'*'});
    if (!roster?.entities || roster.truncated) throw new Error('Lantern roster inspection unavailable or truncated');
    const lanterns = roster.entities.filter(row => row.components?.Lantern).map(row => {
      const position = row.components.Transform?.position;
      if (!row.name || !position) throw new Error('A runtime lantern is missing its authored identity or Transform');
      const [x, y, z] = position;
      return {id:row.name, x, y, z, lit:row.components.Lantern.lit === true};
    });
    let phase = published.phase ?? 'title';
    if (phase === 'title' && args.started === true) phase = 'playing';
    if (world.paused && args.started === true && phase === 'playing') phase = 'paused';
    return {
      phase,
      ticks: world.tick,
      elapsed: Number(session.elapsed ?? world.tick) / 60,
      remaining: Number(published.remaining ?? Math.max(0, 180 - world.tick / 60)),
      player: {
        x: playerAt[0], y: playerAt[1] - .65, z: playerAt[2],
        vx: playerVelocity[0], vy: playerVelocity[1], vz: playerVelocity[2],
        grounded: player?.CapsuleController?.grounded === true,
        animation: fox?.Animation?.clip ?? 'Survey',
      },
      crate: {
        x: crateAt[0], y: crateAt[1], z: crateAt[2],
        vx: crateVelocity[0], vy: crateVelocity[1], vz: crateVelocity[2],
      },
      lanterns,
      count: lanterns.filter(item => item.lit).length,
      total: lanterns.length,
      events: [...eventRows(logs).map(event => ({...event, tick: world.tick})), ...adapterEvents],
      assetReady: true,
    };
  }
  async function restart(started) {
    await releaseInput().catch(() => {});
    await exact().reload(null);
    await waitReady();
    if (started) {
      const play = await viewByTestId('play');
      if (!play) throw new Error('Play action is unavailable');
      play.click();
      await Promise.resolve();
    }
    return state();
  }
  function encode(bytes) {
    let text = '';
    for (let i = 0; i < bytes.length; i += 8192) text += String.fromCharCode(...bytes.subarray(i, i + 8192));
    return btoa(text);
  }
  function decode(text) {
    const binary = atob(text), bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
  }
  async function save() {
    const {view, result: world} = await waitReady();
    const reply = exact().gpu.handle({op: 'screenshot', id: view, form: 'save'}, null, value => value);
    if (!reply?.data) throw new Error(reply?.error ?? 'Exact surface did not provide a save');
    localStorage.setItem(STORAGE_KEY, reply.data);
    adapterEvents.push({type: 'save', tick: world.tick});
    return {saved: true};
  }
  async function load() {
    const encoded = localStorage.getItem(STORAGE_KEY);
    if (!encoded) return {loaded: false};
    const {view, result: world} = await waitReady();
    await releaseInput();
    const args = world.args ?? {};
    if (typeof args.scene !== 'string' || !args.scene) throw new Error('Load needs the retained baked scene construction argument');
    exact().worldCarry = decode(encoded);
    exact().gpu.destroy(view);
    exact().gpu.surface(view, 'world', [
      args.seed ?? 1041003, true, false, args.round ?? 0,
      args.jump_press ?? 0, args.light_press ?? 0, args.sound ?? true, args.scene,
    ]);
    await waitReady();
    const loaded = await state();
    adapterEvents.push({type: 'load', tick: loaded.ticks});
    return loaded;
  }
  async function pause() {
    const current = await state();
    if (!['playing', 'paused'].includes(current.phase)) return current;
    const button = await viewByTestId(current.phase === 'paused' ? 'resume' : 'pause');
    if (!button) throw new Error('Pause action is unavailable');
    button.click();
    await Promise.resolve();
    return state();
  }

  globalThis.lanterns = {
    async command(request = {}) {
      switch (request.op) {
        case 'ready': {
          const result = await readyState();
          return result ? {ready: true, engine: 'exact', version: '1041.003'} : {ready: false};
        }
        case 'state': return state();
        case 'start': return restart(true);
        case 'reset': return restart(false);
        case 'input': await setInput(request); return state();
        case 'step': {
          const ticks = Math.max(0, Math.trunc(Number(request.ticks) || 0));
          const before = await state();
          if (ticks && before.phase === 'playing') {
            const {view} = await waitReady();
            surface(view, {op: 'clock', ticks});
          }
          return state();
        }
        case 'pause': return pause();
        case 'save': return save();
        case 'load': return load();
        default: throw new Error(`Unknown Lanterns command: ${request.op}`);
      }
    },
  };
}
