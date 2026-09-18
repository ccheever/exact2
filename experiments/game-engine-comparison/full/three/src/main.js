import { LanternsGame, FIXED_DT } from './game.js';
import { LanternsView } from './view.js';
import { LanternsUI } from './ui.js';

const game = new LanternsGame();
const ui = new LanternsUI(game);
const view = new LanternsView(document.getElementById('game'), game);
const agentMode = new URLSearchParams(location.search).get('agent') === '1';
let state = game.state();
let lastTime = performance.now() / 1000;
let accumulator = 0;

game.onChange((next) => {
  state = next;
  ui.sync(state);
});

const readyPromise = view.loadFox().then(() => {
  state = game.state();
  ui.sync(state);
  ui.finishLoading();
  return { ready: true, engine: 'Three.js', version: '0.186.0' };
}).catch((error) => {
  console.error('Fox asset failed to load', error);
  document.getElementById('loading').querySelector('span').textContent = 'The fox could not reach the island.';
  throw error;
});

function frame(milliseconds) {
  const now = milliseconds / 1000;
  const frameDelta = Math.min(.25, Math.max(0, now - lastTime));
  lastTime = now;
  if (!agentMode && game.phase === 'playing') {
    accumulator += frameDelta;
    while (accumulator >= FIXED_DT) {
      game.step(1);
      accumulator -= FIXED_DT;
    }
    state = game.state();
  } else if (game.phase !== 'playing') {
    accumulator = 0;
  }
  ui.sync(state);
  view.render(state, frameDelta);
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

window.lanterns = {
  async command(request = {}) {
    await readyPromise;
    switch (request.op) {
      case 'ready': return { ready: true, engine: 'Three.js', version: '0.186.0' };
      case 'start': state = game.start(); break;
      case 'reset': state = game.reset(); break;
      case 'input': state = game.setInput(request); break;
      case 'step': state = game.step(request.ticks ?? 1); break;
      case 'state': state = game.state(); break;
      case 'pause': state = game.togglePause(); break;
      case 'save': return game.save();
      case 'load': state = game.load(); break;
      default: return { error: `Unknown operation: ${String(request.op)}` };
    }
    ui.sync(state);
    return state;
  },
};

window.addEventListener('error', (event) => console.error('Lanterns runtime error', event.error || event.message));
